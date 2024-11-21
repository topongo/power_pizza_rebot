use std::io::Write;
use std::fmt::Display;
use std::sync::{Arc, Mutex};
use log::{error, info, warn};
use futures_util::stream::StreamExt;

use crate::db::{DB, PPPData};
use crate::spreaker::Episode;
use crate::transcript::data::TranscriptAlt;
use tokio::sync::Semaphore;


use tokio::task::JoinHandle;

use super::data::{EpisodeTranscript, Transcript}; type JobContainer<T> = Mutex<Vec<JoinHandle<Result<T, JobManagerError>>>>;

pub struct JobManager {
    cli: Arc<reqwest::Client>,
    conv_sem: Arc<Semaphore>,
    tran_sem: Arc<Semaphore>,
    down_sem: Arc<Semaphore>,
    conv_jobs: JobContainer<EpisodeTranscript>,
    tran_jobs: JobContainer<(u32, Transcript)>,
    down_jobs: JobContainer<u32>,
}

impl JobManager {
    pub fn new(cli: Arc<reqwest::Client>) -> Self {
        Self {
            cli,
            conv_sem: Arc::new(Semaphore::new(MAX_CONVERT_JOBS)),
            tran_sem: Arc::new(Semaphore::new(MAX_TRANSCRIBE_JOBS)),
            down_sem: Arc::new(Semaphore::new(MAX_DOWNLOAD_JOBS)),
            conv_jobs: Mutex::new(vec![]),
            tran_jobs: Mutex::new(vec![]),
            down_jobs: Mutex::new(vec![]),
        }
    }

    pub fn run_convert(&self, id: u32, transcript: Transcript) {
        let conv = Self::_run_convert(id, transcript, self.conv_sem.clone());
        let handle = tokio::spawn(conv);
        self.conv_jobs.lock().unwrap().push(handle);
    }

    pub fn run_transcribe(&self, id: u32) {
        let tran = Self::_run_transcribe(id, self.cli.clone(), self.tran_sem.clone());
        let handle = tokio::spawn(tran);
        self.tran_jobs.lock().unwrap().push(handle);
    }

    pub fn run_download(&self, id: u32) {
        let down = Self::_run_download(id, self.cli.clone(), self.down_sem.clone());
        let handle = tokio::spawn(down);
        self.down_jobs.lock().unwrap().push(handle);
    }

    async fn _run_convert(id: u32, transcript: Transcript, sem: Arc<Semaphore>) -> Result<EpisodeTranscript, JobManagerError> {
        let _permit = sem.acquire().await.unwrap();
        let transcript = (id, transcript).into();
        drop(_permit);
        Ok(transcript)
    }

    async fn _run_transcribe(id: u32, cli: Arc<reqwest::Client>, sem: Arc<Semaphore>) -> Result<(u32, Transcript), JobManagerError> {
        let _permit = sem.acquire().await.unwrap();
        let f = format!("./audio/wav/{}.wav", id);
        // warn!("using file: {}", f);
        info!("transcribing espisode {}", id);
        let t = cli
            .post("http://127.0.0.1:8080/inference")
            .multipart(reqwest::multipart::Form::new()
                .text("temperature", "0.0")
                .text("temperature_inc", "0.0")
                .text("response_format", "verbose_json")
                .file("file", f).await?
            )
        // info!("sending request for episode {}: {:?}", id, req);
        // let t = req
            .send()
            .await.unwrap()
            .json::<TranscriptAlt>()
            .await.unwrap();
        let t: Transcript = t.into();
        let cache = std::fs::File::create(format!("output/{}.json", id))?;
        serde_json::to_writer(cache, &t)?;
        drop(_permit);
        Ok((id, t))
    }
    
    async fn _run_download(id: u32, cli: Arc<reqwest::Client>, sem: Arc<Semaphore>) -> Result<u32, JobManagerError> {
        let _permit = sem.acquire().await.unwrap();
        let e = DB.get::<Episode>(id).await?.unwrap();
        let url = e.download_url;
        let res = cli.get(&url).send().await?;
        let mp3 = format!("./audio/mp3/{}.mp3", e.id);
        let wav = format!("./audio/wav/{}.wav", e.id);
        if res.status().is_success() {
            let mut file = std::fs::File::create(&mp3)?;
            let mut stream =  res.bytes_stream();
            while let Some(item) = stream.next().await {
                let chunk = item?;
                file.write_all(&chunk)?;
            }
        }
        match tokio::process::Command::new("ffmpeg")
            .args(["-i", &mp3, "-ar", "16000", "-ac", "1", "-c:a", "pcm_s16le", &wav])
            .status()
            .await? {
            s if s.success() => {
                std::fs::remove_file(&mp3)?;
            }
            s => {
                error!("couldn't convert episode {} from mp3 to wav: {}", id, s);
            }
        }
        drop(_permit);
        Ok(id)
    }

    pub async fn wait(self) -> Result<Vec<EpisodeTranscript>, JobManagerError> {
        for j in self.down_jobs.into_inner().unwrap().into_iter() {
            let id = j.await??;
            let job = Self::_run_transcribe(id, self.cli.clone(), self.tran_sem.clone());
            self.tran_jobs.lock()?.push(tokio::spawn(job));
        }

        for j in self.tran_jobs.into_inner().unwrap().into_iter() {
            let (id, t) = j.await??;
            let job = Self::_run_convert(id, t, self.conv_sem.clone());
            self.conv_jobs.lock()?.push(tokio::spawn(job));
        }

        let mut out = vec![];
        for j in self.conv_jobs.into_inner().unwrap().into_iter() {
            out.push(j.await??)
        }
        Ok(out)
    }
}

static MAX_CONVERT_JOBS: usize = 4;
static MAX_TRANSCRIBE_JOBS: usize = 1;
static MAX_DOWNLOAD_JOBS: usize = 4;

#[derive(Debug)]
pub enum JobManagerError {
    Reqwest(reqwest::Error),
    Io(std::io::Error),
    Tokio(tokio::task::JoinError),
    Mongo(mongodb::error::Error),
    Mutex,
    Serde(serde_json::Error),
}

impl Display for JobManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Reqwest(e) => write!(f, "Reqwest error: {}", e),
            Self::Io(e) => write!(f, "IO error: {}", e),
            Self::Tokio(e) => write!(f, "Tokio error: {}", e),
            Self::Mutex => write!(f, "Mutex error"),
            Self::Mongo(e) => write!(f, "MongoDB error: {}", e),
            Self::Serde(e) => write!(f, "Serde error: {}", e),
        }
    }

}

impl std::error::Error for JobManagerError {}

impl From<reqwest::Error> for JobManagerError {
    fn from(e: reqwest::Error) -> Self {
        Self::Reqwest(e)
    }
}

impl From<std::io::Error> for JobManagerError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<tokio::task::JoinError> for JobManagerError {
    fn from(e: tokio::task::JoinError) -> Self {
        Self::Tokio(e)
    }
}

impl<T> From<std::sync::PoisonError<T>> for JobManagerError {
    fn from(_: std::sync::PoisonError<T>) -> Self {
        Self::Mutex
    }
}

impl From<mongodb::error::Error> for JobManagerError {
    fn from(e: mongodb::error::Error) -> Self {
        Self::Mongo(e)
    }
}

impl From<serde_json::Error> for JobManagerError {
    fn from(e: serde_json::Error) -> Self {
        Self::Serde(e)
    }
}
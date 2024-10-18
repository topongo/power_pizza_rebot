use reqwest::Client;
use tokio_stream::Stream;
use std::{collections::VecDeque, fmt::Debug, fs::File, future::Future, io::Cursor, path::PathBuf, pin::Pin, sync::{Arc, Mutex}, task::{Context, Poll, Waker}, time::Duration};
use serde::{de::DeserializeOwned, Deserialize};
use tokio::{sync::Notify, task::JoinHandle};
use tokio_stream::StreamExt;
#[allow(unused_imports)]
use log::{info,warn,debug,error};
use lazy_static::lazy_static;

lazy_static! {
    static ref OUTPUT_DIR: PathBuf = PathBuf::from("output");
}

#[derive(Deserialize, Debug)]
struct SpreakerResponse<T> {
    pub response: SpreakerData<T>,
}

#[derive(Deserialize, Debug)]
struct SpreakerData<T> {
    pub items: Vec<T>,
    pub next_url: Option<String>,
}

impl<T> SpreakerData<T> where T: DeserializeOwned + Send + 'static {
    fn request(next_url: String, cli: Arc<Client>) -> SpreakerDataIter<T> {
        SpreakerDataIter::new(cli, next_url)
    }

    async fn _next(next_url: String, cli: &Client) -> Result<SpreakerData<T>, Box<dyn std::error::Error>> {
        Ok(cli.get(next_url)
            .send()
            .await?
            .json()
            .await?)
    }
}

struct SpreakerDataIter<T> {
    cli: Arc<Client>,
    finished: Arc<Mutex<bool>>,
    next_url: String,
    items: Arc<Mutex<VecDeque<T>>>,
    future: Option<JoinHandle<Result<(), SpreakerError>>>,
}

#[derive(Debug)]
enum SpreakerError {
    RequestError(reqwest::Error),
    JsonError(reqwest::Error),
    Runtime(tokio::task::JoinError),
    IOError(std::io::Error),
}

impl<'a, T: 'static> SpreakerDataIter<T> where T: DeserializeOwned + Send {
    fn new(cli: Arc<Client>, next_url: String) -> Self {
        let items = Arc::new(Mutex::new(VecDeque::from(vec![])));
        Self {
            cli,
            items,
            next_url,
            finished: Arc::new(Mutex::new(false)),
            future: None,
        }
    }

    async fn worker(cli: Client, mut next: String, items: Arc<Mutex<VecDeque<T>>>, cx: &Context<'_>) -> Result<(), SpreakerError> {
        
        todo!()
    } 
}

impl<T> Stream for SpreakerDataIter<T> where T: DeserializeOwned + std::marker::Sync + Send + 'static {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.future.is_none() {
            info!("spawning worker");
            let cli = self.cli.clone();
            let mut next = self.next_url.clone();
            let items = self.items.clone();
            let waker = cx.waker().clone();
            self.future = Some(tokio::spawn(async move {
                loop {
                    info!("fetching next url: {}", next);
                    debug!("sending request...");
                    let req = cli.get(&next).send().await.map_err(SpreakerError::RequestError)?;
                    debug!("request sent");
                    debug!("parsing response...");
                    let content = req.text().await.map_err(SpreakerError::RequestError)?;
                    let resp: SpreakerResponse<T> = serde_json::from_str(&content).expect("error parsing json");
                    debug!("response parsed");
                    let resp = resp.response;
                    info!("got response: {} items", resp.items.len());
                    debug!("locking items mutex");
                    let mut lock = items.lock().unwrap();
                    debug!("lock aquired");
                    debug!("extending items");
                    lock.extend(resp.items);
                    debug!("waking context");
                    waker.wake_by_ref();
                    debug!("items extended");
                    match resp.next_url {
                        Some(url) => next = url,
                        None => {
                            debug!("reached end of pagination");
                            break Ok(())
                        }
                    }
                }
            }));
            debug!("worker spawned");
        }

        let mut items = self.items.lock().unwrap();
        if items.is_empty() {
            if self.future.as_ref().unwrap().is_finished() {
                Poll::Ready(None)
            } else {
                Poll::Pending
            }
        } else {
            Poll::Ready(items.pop_front()) 
        }
    }
}

#[derive(Deserialize, Debug)]
struct SimpleEpisode {
    #[serde(rename = "episode_id")]
    id: u32,
    download_url: String,
    title: String,
    #[serde(flatten)]
    remaining: serde_json::Value,
}

struct SpreakerDownloader {
    cli: Arc<Client>,
    queue: Arc<Mutex<VecDeque<SimpleEpisode>>>,
    jobs: usize,
    workers: Arc<Mutex<Vec<JoinHandle<Result<(), SpreakerError>>>>>,
    manager: JoinHandle<Result<(), SpreakerError>>,
    waiting: Arc<Mutex<bool>>,
    wake: Arc<Notify>,
}

impl SpreakerDownloader {
    fn new(cli: Arc<Client>, jobs: usize) -> Self {
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let workers = Arc::new(Mutex::new(vec![]));
        let wake = Arc::new(Notify::new());
        let waiting = Arc::new(Mutex::new(false));
        let manager = tokio::spawn(Self::_manager(
            cli.clone(),
            workers.clone(),
            queue.clone(),
            jobs,
            wake.clone(),
            waiting.clone(),
        ));
        Self {
            cli,
            jobs,
            waiting,
            queue,
            workers,
            manager,
            wake,
        }
    }

    fn download(&self, ep: SimpleEpisode) {
        let mut q = self.queue.lock().unwrap();
        q.push_back(ep);
        info!("pushed episode to queue");
        self.wake.notify_one();
    }

    async fn _manager(
        cli: Arc<Client>,
        workers: Arc<Mutex<Vec<JoinHandle<Result<(), SpreakerError>>>>>,
        queue: Arc<Mutex<VecDeque<SimpleEpisode>>>,
        jobs: usize,
        wake: Arc<Notify>,
        waiting: Arc<Mutex<bool>>,
    ) -> Result<(), SpreakerError> {
        let notify = Arc::new(Notify::new());
        loop {
            while true {
                let nw = workers.lock().unwrap().len();
                debug!("wokers now: {}", nw);
                if nw >= jobs {
                    break
                }

                let ep = queue.lock().unwrap().pop_front();
                match ep {
                    Some(ep) => {
                        info!("spawning download worker for episode {}", ep.id);
                        let cli = cli.clone();
                        let notify = notify.clone();
                        workers
                            .lock()
                            .unwrap()
                            .push(tokio::spawn(Self::_download(cli, ep, notify)));
                    }
                    None => {
                        wake.notified().await;
                        if *waiting.lock().unwrap() {
                            return Ok(())
                        }
                    }
                }
            }
            debug!("waiting for any worker to finish");
            notify.notified().await;
            debug!("removing finished workers");
            let mut lc = 0;
            loop {
                let wn = workers.lock().unwrap().len();
                for j in lc..wn {
                    let mut w = workers.lock().unwrap();
                    if w[j].is_finished() {
                        w.remove(j);
                        lc = j;
                        break
                    }
                }
                if lc == wn - 1 {
                    break
                }
            }
        }
    } 

    async fn _download(cli: Arc<Client>, ep: SimpleEpisode, notify: Arc<Notify>) -> Result<(), SpreakerError> {
        let ep_id = ep.id;
        match Self::_download_inner(cli, ep).await {
            Ok(_) => {
                info!("episode {} downloaded", ep_id);
                notify.notify_one();
                Ok(())
            }
            Err(e) => {
                error!("error downloading episode {}: {:?}", ep_id, e);
                notify.notify_one();
                Err(e)
            }
        }
    }

    async fn _download_inner(cli: Arc<Client>, ep: SimpleEpisode) -> Result<(), SpreakerError> {
        info!("starting downlod for episode {}", ep.id);
        let req = cli.get(&ep.download_url).send().await.map_err(SpreakerError::RequestError)?;
        let output = OUTPUT_DIR.join(format!("{} - {}.mp3", ep.id, ep.title));
        if output.exists() && output.is_file() && output.metadata().unwrap().len() == req.content_length().unwrap() {
            info!("episode {} already downloaded", ep.id);
            return Ok(())
        }
        let mut file = File::create(output).map_err(SpreakerError::IOError)?;
        let mut content = Cursor::new(req.bytes().await.map_err(SpreakerError::RequestError)?);
        std::io::copy(&mut content, &mut file).map_err(SpreakerError::IOError)?;

        debug!("worker for episode {} finished", ep.id);
        Ok(())
    }
    
    async fn join(self) -> Result<(), SpreakerError> {
        *self.waiting.lock().unwrap() = true;
        self.manager.await.map_err(SpreakerError::Runtime)?
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    pretty_env_logger::init();
    let cli = Arc::new(Client::new());
    
    // let eps = SpreakerData::<SimpleEpisode>::request(
    //     "https://api.spreaker.com/v2/shows/3039391/episodes".to_string(),
    //     cli.clone(),
    // ).await?;

    let mut it = SpreakerData::<SimpleEpisode>::request(
        "https://api.spreaker.com/v2/shows/3039391/episodes".to_owned(),
        cli.clone(),
    );
    let mut downloader = SpreakerDownloader::new(cli, 10);
    while let Some(e) = it.next().await {
        downloader.download(e);
    }
    downloader.join().await;
    Ok(())
}

use reqwest::Client;
use std::{collections::VecDeque, fs::File, io::Write, path::PathBuf, sync::{Arc, Mutex}};
use tokio::{sync::Notify, task::JoinHandle};
use tokio_stream::StreamExt;
#[allow(unused_imports)]
use log::{info,warn,debug,error,trace};

use super::{error::SpreakerError, simple_episode::SimpleEpisode};

pub struct SpreakerDownloader {
    queue: Arc<Mutex<VecDeque<SimpleEpisode>>>,
    manager: JoinHandle<Result<(), SpreakerError>>,
    waiting: Arc<Mutex<bool>>,
    wake: Arc<Notify>,
}

type Worker = JoinHandle<Result<(), SpreakerError>>;

impl SpreakerDownloader {
    pub fn new(cli: Arc<Client>, jobs: usize, output: PathBuf) -> Self {
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
            output.clone(),
        ));
        Self {
            waiting,
            queue,
            manager,
            wake,
        }
    }

    pub fn download(&self, ep: SimpleEpisode) {
        let mut q = self.queue.lock().unwrap();
        q.push_back(ep);
        debug!("pushed episode to queue");
        self.wake.notify_one();
    }

    async fn _manager(
        cli: Arc<Client>,
        workers: Arc<Mutex<Vec<Worker>>>,
        queue: Arc<Mutex<VecDeque<SimpleEpisode>>>,
        jobs: usize,
        wake: Arc<Notify>,
        waiting: Arc<Mutex<bool>>,
        output: PathBuf,
    ) -> Result<(), SpreakerError> {
        let notify = Arc::new(Notify::new());
        let output = Arc::new(output);
        loop {
            loop {
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
                            .push(tokio::spawn(Self::_download(cli, ep, notify, output.clone())));
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

    async fn _download(cli: Arc<Client>, ep: SimpleEpisode, notify: Arc<Notify>, output: Arc<PathBuf>) -> Result<(), SpreakerError> {
        let ep_id = ep.id;
        match Self::_download_inner(cli, ep, output).await {
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

    async fn _download_inner(cli: Arc<Client>, ep: SimpleEpisode, output: Arc<PathBuf>) -> Result<(), SpreakerError> {
        info!("starting downlod for episode {}", ep.id);
        let req = cli.get(&ep.download_url).send().await.map_err(SpreakerError::RequestError)?;
        debug!("generated request for episode {}", ep.id);
        let output = output.join(format!("{} - {}.mp3", ep.id, ep.title));
        if output.exists() && output.is_file() && output.metadata().unwrap().len() == req.content_length().unwrap() {
            info!("episode {} already downloaded", ep.id);
            return Ok(())
        }
        let mut file = File::create(output).map_err(SpreakerError::IOError)?;
        let mut res = req.bytes_stream();
        while let Some(v) = res.next().await {
            let v = v.map_err(SpreakerError::RequestError)?;
            trace!("writing chunk {}", v.len());
            file.write(&v).map_err(SpreakerError::IOError)?;
        }
        debug!("worker for episode {} finished", ep.id);
        Ok(())
    }
    
    pub async fn join(self) -> Result<(), SpreakerError> {
        *self.waiting.lock().unwrap() = true;
        self.manager.await.map_err(SpreakerError::Runtime)?
    }
}

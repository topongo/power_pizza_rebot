use std::{collections::VecDeque, pin::Pin, sync::{Arc, Mutex}, task::{Context, Poll}};

#[allow(unused_imports)]
use log::{info,debug,warn,error};
use reqwest::Client;
use serde::{Deserialize, de::DeserializeOwned};
use tokio::task::JoinHandle;
use tokio_stream::Stream;
use super::{error::SpreakerError, SpreakerResponse};


pub struct SpreakerDataIter<T> {
    cli: Arc<Client>,
    finished: Arc<Mutex<bool>>,
    next_url: String,
    items: Arc<Mutex<VecDeque<T>>>,
    future: Option<JoinHandle<Result<(), SpreakerError>>>,
}

impl<'a, T: 'static> SpreakerDataIter<T> where T: DeserializeOwned + Send {
    pub(crate) fn new(cli: Arc<Client>, next_url: String) -> Self {
        let items = Arc::new(Mutex::new(VecDeque::from(vec![])));
        Self {
            cli,
            items,
            next_url,
            finished: Arc::new(Mutex::new(false)),
            future: None,
        }
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

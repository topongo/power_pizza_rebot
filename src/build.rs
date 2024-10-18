#![feature(async_iterator)]
use log::{debug, warn};
use reqwest::Client;
use tokio_stream::Stream;
use std::{async_iter::AsyncIterator, collections::VecDeque, fmt::Debug, pin::Pin, sync::{Arc, Mutex}, task::{Context, Poll}};
use serde::{de::DeserializeOwned, Deserialize};
use tokio_stream::StreamExt;

// {
//        "episode_id": 56245683,
//        "type": "RECORDED",
//        "title": "248: Tornado Potato (con CKibe)",
//        "duration": 3469130,
//        "explicit": false,
//        "show_id": 3039391,
//        "author_id": 10653829,
//        "image_url": "https://d3wo5wojvuv7l.cloudfront.net/t_square_limited_160/images.spreaker.com/original/e40c99acb945dbfb7145e2fc23663de2.jpg",
//        "image_original_url": "https://d3wo5wojvuv7l.cloudfront.net/images.spreaker.com/original/e40c99acb945dbfb7145e2fc23663de2.jpg",
//        "image_transformation": "square_limited_100",
//        "published_at": "2023-07-28 08:00:02",
//        "auto_published_at": null,
//        "download_enabled": true,
//        "stream_id": null,
//        "waveform_url": "https://d3770qakewhkht.cloudfront.net/episode_56245683.gz.json?v=UvNvmA",
//        "slug": "248-tornado-potato-con-ckibe",
//        "site_url": "https://www.spreaker.com/episode/248-tornado-potato-con-ckibe--56245683",
//        "download_url": "https://api.spreaker.com/v2/episodes/56245683/download.mp3",
//        "playback_url": "https://api.spreaker.com/v2/episodes/56245683/play.mp3"
//      }

// #[derive(Deserialize, Debug)]
// enum SpreakerData<'a, T> {
//     Success {
//         items: Vec<Box<&'a dyn SpreakerType<'a, T>>>,
//         next_url: String,
//     },
//     Error {
//     }
// }

#[derive(Deserialize, Debug)]
struct SpreakerResponse<T> {
    pub response: SpreakerData<T>,
}

#[derive(Deserialize, Debug)]
struct SpreakerData<T> {
    pub items: Vec<T>,
    pub next_url: String,
}

impl<T> SpreakerData<T> where T: DeserializeOwned + Send + 'static {
    async fn request(next_url: String, cli: Client) -> Result<SpreakerData<T>, Box<dyn std::error::Error>> {
        let res = cli.get(&next_url)
            .send()
            .await?
            .text()
            .await?;

        debug!("response: {:?}", res);

        let res: SpreakerResponse<T> = serde_json::from_str(&res)?;
        Ok(res.response)
        // Ok(cli.get(next_url)
        //     .send()
        //     .await?
        //     .json()
        //     .await?)
    }

    fn chain_to_end(self, cli: &Client) -> SpreakerDataIter<T> {
        SpreakerDataIter::new(cli.clone(), self)
    }
}

struct SpreakerDataIter<T> {
    finished: Arc<Mutex<bool>>,
    next_url: String,
    items: Arc<Mutex<VecDeque<T>>>,
}

enum SpreakerError {
    RequestError(reqwest::Error),
    JsonError(reqwest::Error),
}

impl<'a, T: 'static> SpreakerDataIter<T> where T: DeserializeOwned + Send {
    fn new(cli: Client, start: SpreakerData<T>) -> Self {
        let items = Arc::new(Mutex::new(VecDeque::from(start.items)));
        Self {
            items,
            next_url: start.next_url,
            finished: Arc::new(Mutex::new(false)),
        }
    }

    fn start_worker(s: &Arc<Self>, cli: Client) {
        let this = Arc::clone(s);
        tokio::spawn(async move { this.worker(cli, this.next_url.clone()).await });
    }

    async fn worker(&self, cli: Client, next: String) -> Result<(), SpreakerError> {
        loop {
            let req = cli.get(&next).send().await.map_err(SpreakerError::RequestError)?;
            let resp: SpreakerData<T> = req.json().await.map_err(SpreakerError::JsonError)?;
            self.items.lock().unwrap().extend(resp.items);
            let next = resp.next_url;
            println!("next: {:?}", next);
            if next.is_empty() {
                break Ok(());
            }
        }
    } 
}

impl<T> Stream for SpreakerDataIter<T> where T: DeserializeOwned + std::marker::Sync {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.items.lock().unwrap().is_empty() {
            Poll::Pending
        } else {
            let mut items = self.items.lock().unwrap();
            Poll::Ready(items.pop_front()) 
        }
    }
}

// impl<T> Stream for SpreakerDataIter<T> where T: DeserializeOwned {
//     type Item = T;
//
//     fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
//         if self.current.items.is_empty() {
//             self.current = match await self.current.next_response(&self.cli) {
//                 Ok(next) => next,
//                 Err(e) => {
//                     warn!("Error fetching next response: {:?}", e);
//                     return None;
//                 }
//             };
//         }
//
//         todo!();
//     }
// }

// impl<'a, T> AsyncIterator for SpreakerDataIter<T> where T: DeserializeOwned {
//     type Item = T;
//
//     fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Option<Self::Item> {
//         if self.current.items.is_empty() {
//             self.current = match await self.current.next_response(&self.cli) {
//                 Ok(next) => next,
//                 Err(e) => {
//                     warn!("Error fetching next response: {:?}", e);
//                     return None;
//                 }
//             };
//         }
//
//         todo!();
//     }
// }

#[derive(Deserialize, Debug)]
struct SimpleEpisode {
    #[serde(rename = "episode_id")]
    id: u32,
}

#[tokio::main]
async fn main() -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    pretty_env_logger::init();
    let cli = Client::new();
    
    let eps = SpreakerData::<SimpleEpisode>::request(
        "https://api.spreaker.com/v2/shows/3039391/episodes".to_string(),
        cli.clone(),
    ).await?;
    let it = Arc::new(eps.chain_to_end(&cli));
    SpreakerDataIter::start_worker(&it, cli.clone());
    while let Some(ep) = it.next().await {
        println!("{:?}", ep.id);
    }

    Ok(())
}

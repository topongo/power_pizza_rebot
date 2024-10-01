#![feature(async_iterator)]
use log::{debug, warn};
use reqwest::Client;
use tokio_stream::Stream;
use std::{async_iter::AsyncIterator, fmt::Debug, pin::Pin, task::{Context, Poll}};
use serde::{de::DeserializeOwned, Deserialize};

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
// enum SpreakerResponse<'a, T> {
//     Success {
//         items: Vec<Box<&'a dyn SpreakerType<'a, T>>>,
//         next_url: String,
//     },
//     Error {
//     }
// }

#[derive(Deserialize, Debug)]
struct SpreakerResponse<T> {
    pub items: Vec<T>,
    pub next_url: String,
}

impl<T> SpreakerResponse<T> where T: DeserializeOwned {
    async fn request(next_url: String, cli: Client) -> Result<SpreakerResponse<T>, Box<dyn std::error::Error>> {
        let res = cli.get(&next_url)
            .send()
            .await?
            .text()
            .await?;

        debug!("response: {:?}", res);

        let res: SpreakerResponse<T> = serde_json::from_str(&res)?;
        Ok(res)
        // Ok(cli.get(next_url)
        //     .send()
        //     .await?
        //     .json()
        //     .await?)
    }

    fn chain_to_end(self, cli: &Client) -> SpreakerResponseIter<T> {
        SpreakerResponseIter {
            cli: cli.clone(),
            current: self,
        }
    }
}

struct SpreakerResponseIter<T> {
    cli: Client,
    current: SpreakerResponse<T>, 
}

impl<T> Stream for SpreakerResponseIter<T> where T: DeserializeOwned + std::marker::Sync + 'static {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.current.items.is_empty() {
            let fut = SpreakerResponse::<T>::request(self.current.next_url.clone(), self.cli.clone());
            let waker = cx.waker().clone();
            println!("spawning task");
            tokio::spawn(async move {
                match fut.await {
                    Ok(next) => {
                        println!("task completed");
                        waker.wake();
                    },
                    Err(e) => {
                        warn!("Error fetching next response: {:?}", e);
                        waker.wake();
                    }
                }
            });
        }

        todo!();
    }
}

// impl<T> Stream for SpreakerResponseIter<T> where T: DeserializeOwned {
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

// impl<'a, T> AsyncIterator for SpreakerResponseIter<T> where T: DeserializeOwned {
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
    id: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    pretty_env_logger::init();
    let cli = Client::new();
    
    let eps = SpreakerResponse::<SimpleEpisode>::request(
        "https://api.spreaker.com/v2/shows/3039391/episodes".to_string(),
        cli.clone(),
    ).await?;
    // let eps: SpreakerResponse<SimpleEpisode> = cli.get()
    //     .send()
    //     .await?
    //     .json()
    //     .await?;

    println!("{:?}", eps);
    Ok(())
}

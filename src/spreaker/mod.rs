mod error;
mod downloader;
mod episode;
mod simple_episode;
mod paginator;

pub use error::SpreakerError;
pub use downloader::SpreakerDownloader;
pub use episode::{ProtoEpisode, Episode};
pub use simple_episode::SimpleEpisode;

use std::sync::Arc;
use paginator::SpreakerDataIter;
use reqwest::Client;
use serde::{Deserialize, de::DeserializeOwned};

pub const API_URL: &str = "https://api.spreaker.com/v2";

#[derive(Deserialize, Debug)]
pub struct SpreakerResponse<T> {
    pub response: SpreakerData<T>,
}

#[derive(Deserialize, Debug)]
pub struct SpreakerData<T> {
    pub items: Vec<T>,
    pub next_url: Option<String>,
}


impl<T> SpreakerData<T> where T: DeserializeOwned + Send + 'static {
    pub fn request(next_url: String, cli: Arc<Client>) -> SpreakerDataIter<T> {
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

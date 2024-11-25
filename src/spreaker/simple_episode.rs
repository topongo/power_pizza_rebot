use serde::Deserialize;

use super::{ProtoEpisode, SpreakerError, API_URL};
use super::episode::{Episode, EpisodeResponse};

#[derive(Deserialize, Debug)]
pub struct SimpleEpisode {
    #[serde(rename = "episode_id")]
    pub id: u32,
    pub download_url: String,
    pub title: String,
    #[serde(flatten)]
    pub remaining: serde_json::Value,
}


impl SimpleEpisode {
    pub async fn fetch(id: u32) -> Result<Self, SpreakerError> {
        let resp = reqwest::get(format!("{}/episodes/{}", API_URL, id)).await?.json::<EpisodeResponse<SimpleEpisode>>().await?;
        Ok(resp.into_inner())
    }

    pub async fn get_episode(&self) -> Result<Episode, SpreakerError> {
        let resp = reqwest::get(format!("{}/episodes/{}", API_URL, self.id)).await?.json::<EpisodeResponse<ProtoEpisode>>().await?;
        Ok(resp.into_inner())
    }
}

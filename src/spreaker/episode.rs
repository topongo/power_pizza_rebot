use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};

use crate::db::{PPPData, PPPDatabase};

use super::SimpleEpisode;

#[derive(Deserialize, Debug, Serialize)]
pub struct Episode {
    #[serde(alias = "episode_id")]
    pub id: u32,
    pub title: String,
    pub duration: u32,
    pub show_id: u32,
    pub author_id: u32,
    #[serde(with = "crate::serde::naive_datetime")]
    pub published_at: DateTime<Utc>,
    pub download_url: String,
    pub description: String,
    pub description_html: String,
}

impl PPPData for Episode {
    const COLLECTION: &'static str = "episodes";
    const ID_KEY: &'static str = "id";
    type IdType = Episode;
}


#[derive(Deserialize, Debug)]
pub struct EpisodeResponse<T> {
    response: EpisodeEpisode<T>,
}

#[derive(Deserialize, Debug)]
struct EpisodeEpisode<T> {
    episode: T,
}

impl EpisodeResponse<Episode> {
    pub fn into_inner(self ) -> Episode {
        self.response.episode
    }
}

impl EpisodeResponse<SimpleEpisode> {
    pub fn into_inner(self ) -> SimpleEpisode {
        self.response.episode
    }
}

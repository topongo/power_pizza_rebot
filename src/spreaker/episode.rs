use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::db::PPPData;

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

#[derive(Deserialize)]
pub struct ProtoEpisode {
    pub episode_id: u32,
    pub title: String,
    pub duration: u32,
    pub show_id: u32,
    pub author_id: u32,
    pub published_at: String,
    pub download_url: String,
    pub description: String,
    pub description_html: String,
}

impl From<ProtoEpisode> for Episode {
    fn from(p: ProtoEpisode) -> Self {
        let date = NaiveDateTime::parse_from_str(p.published_at.as_str(), "%Y-%m-%d %H:%M:%S").unwrap();
        Self {
            id: p.episode_id,
            title: p.title,
            duration: p.duration,
            show_id: p.show_id,
            author_id: p.author_id,
            published_at: date.and_utc(),
            download_url: p.download_url,
            description: p.description,
            description_html: p.description_html,
        }
    }
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

impl EpisodeResponse<ProtoEpisode> {
    pub fn into_inner(self ) -> Episode {
        self.response.episode.into()
    }
}

impl EpisodeResponse<SimpleEpisode> {
    pub fn into_inner(self ) -> SimpleEpisode {
        self.response.episode
    }
}

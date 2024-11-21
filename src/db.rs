use std::{cmp::{max, min}, collections::{HashMap, VecDeque}, sync::{Arc, Mutex}, time::Duration};

use chrono::{DateTime, Local, NaiveDateTime};
use futures_util::lock::MutexGuard;
use lazy_static::lazy_static;
use log::{debug, info};
use mongodb::{bson::{doc, from_document, Document}, options::{ClientOptions, Credential, IndexOptions, ServerAddress}, results::CreateIndexResult, Client, Database, IndexModel, SearchIndexModel};
use futures_util::stream::{StreamExt, TryStreamExt};
use futures_util::FutureExt;
use regex::{Regex, RegexBuilder};
use serde::{de::DeserializeOwned, Serialize};
use substring::Substring;
use unidecode::unidecode;

use crate::{spreaker::Episode, status::Status, transcript::EpisodeTranscript};

pub struct PPPDatabase {
    db: Database,
    status: Mutex<Option<Status>>,
}

pub trait PPPData: Serialize + DeserializeOwned + std::marker::Send + std::marker::Sync {
    const COLLECTION: &'static str;
    const ID_KEY: &'static str;
    type IdType: DeserializeOwned + std::marker::Send + std::marker::Sync;
}

impl PPPDatabase {
    pub fn new() -> Self {
        let db = get_client().expect("Failed to connect to MongoDB");
        Self {
            db,
            status: Mutex::new(None),
        }
    }

    pub async fn ensure_index(&self) -> Result<CreateIndexResult, mongodb::error::Error> {
        self.db
            .collection::<()>("transcripts")
            .create_index(IndexModel::builder()
            .keys(doc!{"data": "text"})
            .options(IndexOptions::builder().default_language("italian".to_owned()).build())
            .build()
        ).await
    }

    pub async fn _ensure_status(&self) {
        if self.status.lock().unwrap().is_none() {
            *self.status.lock().unwrap() = Status::from_db(&self.db).await.expect("Failed to get status from db");
        }
    }

    pub async fn _update_status(&self) {
        if self.status.lock().unwrap().is_none() {
            *self.status.lock().unwrap() = Some(Status::default());
            self.db
                .collection::<Status>("status")
                .insert_one(self.status.lock().unwrap().as_ref().unwrap())
                .await
                .expect("Failed to update status in db");
        } else {
            self.db
                .collection::<Status>("status")
                .replace_one(doc!{}, self.status.lock().unwrap().as_ref().unwrap())
                .await
                .expect("Failed to update status in db");
        }
    }

    pub async fn last_modified(&self) -> Option<DateTime<Local>> {
        self._ensure_status().await;
        self.status.lock().unwrap().as_ref().map(|s| s.last_update.clone().with_timezone(&Local))
    }

    pub async fn get_ids<T>(&self) -> Result<Vec<u32>, mongodb::error::Error> where T: PPPData {
        self._ensure_status().await;
        self.db
            .collection::<T::IdType>(T::COLLECTION)
            .aggregate(vec![
                doc!{"$match": {}},
                doc!{"$project": {"_id": 0, T::ID_KEY: 1}},
            ])
            .await?
            .map(|d| d.unwrap().get_i64(T::ID_KEY).map(|v| v as u32))
            .try_collect::<Vec<u32>>()
            .await
            .map_err(mongodb::error::Error::custom)
    }

    pub async fn get<T>(&self, id: u32) -> Result<Option<T>, mongodb::error::Error> where T: PPPData {
        debug!("get {} from collection {} from db", id, T::COLLECTION);
        self._ensure_status().await;
        self.db
            .collection::<T>(T::COLLECTION)
            .find_one(doc!{T::ID_KEY: id})
            .await
    }

    pub async fn insert_stateless<T>(&self, data: &[T]) -> Result<(), mongodb::error::Error> where T: PPPData {
        self._ensure_status().await;
        self.db
            .collection::<T>(T::COLLECTION)
            .insert_many(data)
            .await
            .map(|_| ())
    }

    pub async fn insert_stateful<T>(&self, data: &[T]) -> Result<(), mongodb::error::Error> where T: PPPData {
        self._ensure_status().await;
        self.insert_stateless(data).await?;
        self._update_status().await;
        Ok(())
    }

    pub async fn search_text(&self, text: String) -> Result<Vec<SearchResult>, mongodb::error::Error> {
        self._ensure_status().await;
        let episodes = self.db
            .collection::<EpisodeTranscript>("transcripts")
            .aggregate(vec![
                doc!{"$match": {"$text": {"$search": text}}},
                doc!{"$project": {"episode_id": 1, "_id": 0}},
                doc!{"$sort": {"sort": {"$meta": "textScore"}}},
                doc!{"$lookup": {"from": "episodes", "localField": "episode_id", "foreignField": "id", "as": "episodeDetails"}},
                doc!{"$unwind": "$episodeDetails"},
                doc!{"$replaceRoot": {"newRoot": "$episodeDetails"}},
            ])
            .await?
            .map(|d| d.map(|d| from_document::<Episode>(d.clone()).unwrap()))
            .try_collect::<Vec<Episode>>()
            .await?;

        Ok(episodes.into_iter().map(|episode| SearchResult { episode }).collect())
    }
    
    pub async fn search_transcript_offset(&self, id: u32, text: String) -> Result<Option<OffsetSearchResult>, mongodb::error::Error> {
        self._ensure_status().await;
        let transcript = match self.db
            .collection::<EpisodeTranscript>("transcripts")
            .find_one(doc!{"episode_id": id})
            .await? {
                Some(t) => t,
                None => return Ok(None),
            };

        const HINT_RADIUS: usize = 50;

        let r = RegexBuilder::new(&text)
            .case_insensitive(true)
            .build()
            .expect("Failed to build regex");
        
        let data = unidecode(&transcript.data);
        let mut matches = VecDeque::new();
        for pos in r.find_iter(&data) {
            let pos=  pos.start();
            matches.push_back((pos, data.substring(max(0, pos - HINT_RADIUS / 2), min(transcript.data.len(), pos + text.len() + HINT_RADIUS / 2)).to_string()));
        }
        Ok(Some(OffsetSearchResult::from(matches, transcript.timestamps)))
    }
}

#[derive(Debug)]
pub struct SearchResult {
    pub episode: Episode,
}

pub struct OffsetSearchResult {
    pub matches: Vec<EpisodeOffsetMatch>,
}

impl OffsetSearchResult {
    pub fn from(mut matches: VecDeque<(usize, String)>, timestamps: Vec<(Duration, usize)>) -> Self {
        let mut curr = match matches.pop_front() {
            Some(m) => m,
            None => return Self { matches: vec![] },
        };
        let mut out = vec![];
        let mut prev = 0;
        let mut prev_time = Duration::from_secs(0);
        for (time, offset) in timestamps {
            if curr.0 > prev && curr.0 < offset {
                out.push(EpisodeOffsetMatch {
                    time: time,
                    hint: curr.1,
                });
                curr = match matches.pop_front() {
                    Some(m) => m,
                    None => break,
                };
            }
            prev = offset;
            prev_time = time;
        }
        Self {
            matches: out,
        }
    }
}

pub struct EpisodeOffsetMatch {
    pub time: Duration,
    pub hint: String,
}

/// # Queries:
/// Get audio timestamp from text offset
/// db.transcripts.aggregate([{$match: {episode_id: 56245683}}, {$project: {index: {$indexOfCP: ["$data", "Undertale"]}, timestamps: 1}}, {$unwind: "$timestamps"}, {$match: {"timestamps.1": {$lte: 52434}}}, {$sort: {"timestamps.1": -1}}, {$limit: 1}])
///
/// Get episode id from search string
/// db.transcripts.aggregate([{ $match: {$text: {$search: "undertale"} }}, {$project: {episode_id: 1, _id: 0}}, {$lookup: {from: "episodes", localField: "episode_id", foreignField: "id", as: "episodeDetails"}}, {$project: {name: "$episodeDetails.title", id: "$episode_id"}}])

fn get_client() -> Result<Database , Box<dyn std::error::Error>> {
    let options = ClientOptions::builder()
        .hosts(vec![
            ServerAddress::Tcp {
                host: std::env::var("PPP_MONGO_HOST").unwrap_or("localhost".to_owned()),
                port: Some(std::env::var("PPP_MONGO_PORT").unwrap_or("27017".to_owned()).parse()?),
            },
        ])
        .credential(Credential::builder()
            .username(std::env::var("PPP_MONGO_USER").unwrap_or("ppp".to_owned()))
            .password(std::env::var("PPP_MONGO_PASSWORD")?)
            .build()
        )
        .build();

    Ok(Client::with_options(options)?.database("ppp"))
}

lazy_static! {
    pub static ref DB: PPPDatabase = PPPDatabase::new();
}

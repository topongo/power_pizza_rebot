use std::{cmp::{min,max}, collections::VecDeque, time::Instant};
use futures_util::{StreamExt, TryStreamExt};
use log::{debug, trace};
use mongodb::bson::{doc, from_document};
use regex::bytes::RegexBuilder;
use substring::Substring;
use unidecode::unidecode;

use crate::{db::PPPDatabase, spreaker::Episode, transcript::{EpisodeTranscript, FromTo, Timestamp}};

/// # Queries:
/// Get audio timestamp from text offset
/// db.transcripts.aggregate([{$match: {episode_id: 56245683}}, {$project: {index: {$indexOfCP: ["$data", "Undertale"]}, timestamps: 1}}, {$unwind: "$timestamps"}, {$match: {"timestamps.1": {$lte: 52434}}}, {$sort: {"timestamps.1": -1}}, {$limit: 1}])
///
/// Get episode id from search string
/// db.transcripts.aggregate([{ $match: {$text: {$search: "undertale"} }}, {$project: {episode_id: 1, _id: 0}}, {$lookup: {from: "episodes", localField: "episode_id", foreignField: "id", as: "episodeDetails"}}, {$project: {name: "$episodeDetails.title", id: "$episode_id"}}])
impl PPPDatabase {
    /// Perform a full-text search across all transcripts in the database.
    /// Returns a list of episodes in which the search string was found.
    /// It does not return the actual matches nor the timestamps, take a look at `search_transcript_one` for that.
    pub async fn search_transcript_all(&self, text: String) -> Result<Vec<SearchResult>, SearchError> {
        self._ensure_status().await;
        let _t = Instant::now();
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
            // unwrap safe: as long as the schema and query are correct, this should not fail after this point
            .map(|d| d.map(|d| from_document::<Episode>(d.clone()).unwrap()))
            .try_collect::<Vec<Episode>>()
            .await?;
        if episodes.is_empty() {
            Err(SearchError::NoResults)
        } else {
            let r = Ok(episodes.into_iter().map(|episode| SearchResult { episode }).collect());
            trace!("timings: search_text: {:?}", _t.elapsed());
            r
        }
    }
    
    /// Perform a full-text regex based search across a single transcript.
    /// Returns a list of matches with their timestamps and text in the neighborhood of the match for context.
    pub async fn search_transcript_one(&self, id: u32, text: String) -> Result<OffsetSearchResult, SearchError> {
        self._ensure_status().await;
        let _t = Instant::now();
        let e = self.get::<Episode>(id).await?.ok_or(SearchError::EpisodeNotFound(id))?;
        let transcript = match self.db
            .collection::<EpisodeTranscript>("transcripts")
            .find_one(doc!{"episode_id": id})
            .await? {
                Some(t) => t,
                None => return Err(SearchError::EpisodeNotFound(id)),
            };


        let r = RegexBuilder::new(&text)
            .case_insensitive(true)
            .build()
            .map_err(SearchError::Regex)?;
        
        let data = unidecode(&transcript.data);
        let mut matches = VecDeque::new();
        for pos in r.find_iter(data.as_ref()) {
            matches.push_back(pos.start());
        }
        if matches.is_empty() {
            return Err(SearchError::NoResults);
        }
        let r = Ok(OffsetSearchResult::from(e, matches, transcript.timestamps, &transcript.data));
        trace!("timings: search_transcript_offset: {:?}", _t.elapsed());
        r
    }

    pub async fn search_meta(&self, text: String) -> Result<Vec<SearchResult>, SearchError> {
        let r = mongodb::bson::Regex { pattern: regex::escape(&text), options: "i".to_string() };
        let res = self.db
            .collection::<Episode>("episodes")
            .find(doc!{"$or": [{"title": r.clone()}, {"description": r.clone()}]})
            .await?
            .map(|d| d.map(|d| SearchResult { episode: d }))
            .try_collect::<Vec<SearchResult>>()
            .await?;
        if res.is_empty() {
            Err(SearchError::NoResults)
        } else {
            Ok(res)
        }
    }

    /// Perform a search for a specific episode by its id/name/number or Magic Identifier™.
    /// Returns an optional u32 representing the episode id.
    pub async fn magic_episode_search(&self, query: String) -> Result<u32, SearchError> {
        match query.parse::<u32>() {
            Ok(num) => {
                debug!("parsed number: {}", num);
                if num > 10000 {
                    debug!("assuming this is an episode id");
                    // we assume that this is the episode id
                    Ok(num)
                } else {
                    debug!("assuming this is an episode number, searching by title");
                    // we assume that this is the episode number
                    self.db
                        .collection::<Episode>("episodes")
                        .find_one(doc!{"title": mongodb::bson::Regex { pattern: num.to_string(), options: "i".to_string() }})
                        .await?
                        .map(|e| e.id)
                        .ok_or(SearchError::NoResults)
                }
            }
            Err(_) => {
                debug!("not a number, searching by title");
                self.db
                    .collection::<Episode>("episodes")
                    .find_one(doc!{"title": mongodb::bson::Regex { pattern: query, options: "i".to_string() }})
                    .await?
                    .map(|e| e.id)
                    .ok_or(SearchError::NoResults)
            }
        }
    }
}

#[derive(Debug)]
pub struct SearchResult {
    pub episode: Episode,
}

#[derive(Debug)]
pub struct OffsetSearchResult {
    pub matches: Vec<EpisodeOffsetMatch>,
    pub episode: Episode,
}

impl OffsetSearchResult {
    pub fn from(episode: Episode, mut input: VecDeque<usize>, timestamps: Vec<Timestamp>, data: &String) -> Self {
        const HINT_RADIUS: usize = 50;

        let mut curr = match input.pop_front() {
            Some(m) => m,
            None => return Self { episode, matches: vec![] },
        };
        let mut matches = vec![];
        // let mut prev_time = Duration::from_secs(0);
        'a: for Timestamp { time, offsets } in timestamps {
            debug!("checking timestamp: Timestamp {{ time: {:?}, offsets: {:?} }}", time, offsets);
            loop {
                if curr >= offsets.0 && curr < offsets.1 {
                    let m = EpisodeOffsetMatch {
                        time: time.clone(),
                        hint: data.substring(max(0, curr as isize - HINT_RADIUS as isize) as usize, min(data.len(), curr + HINT_RADIUS)).to_string(),
                    };
                    debug!("found match: {:?}", m);
                    matches.push(m);
                    curr = match input.pop_front() {
                        Some(m) => m,
                        None => break 'a,
                    };
                } else if curr < offsets.0 && curr < offsets.1 {
                    panic!("this should never happen... curr: {:?}, offsets: {:?}", curr, offsets);
                } else { break }
            }
        }
        Self {
            episode,
            matches,
        }
    }

    pub fn len(&self) -> usize {
        self.matches.len()
    }
}

#[derive(Debug)]
pub struct EpisodeOffsetMatch {
    pub time: FromTo,
    pub hint: String,
}

#[derive(Debug)]
pub enum SearchError {
    EpisodeNotFound(u32),
    Mongo(mongodb::error::Error),
    Regex(regex::Error),
    NoResults,
}

impl From<mongodb::error::Error> for SearchError {
    fn from(e: mongodb::error::Error) -> Self {
        SearchError::Mongo(e)
    }
}

impl From<regex::Error> for SearchError {
    fn from(e: regex::Error) -> Self {
        SearchError::Regex(e)
    }
}

impl SearchError {
    pub fn respond_client(&self) -> &str {
        match self {
            SearchError::EpisodeNotFound(_) => "l'episodio richiesto non esiste",
            SearchError::Mongo(_) => "errore del database",
            SearchError::Regex(_) => "errore nella query",
            SearchError::NoResults => "nessun risultato trovato",
        }
    }
}

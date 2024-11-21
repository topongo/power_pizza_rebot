use std::sync::Mutex;
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use mongodb::{bson::doc, Database};
use serde::{Serialize, Deserialize};

use crate::db::DB;

#[derive(Debug, Serialize, Deserialize)]
pub struct Status {
    #[serde(with = "chrono::serde::ts_seconds")]
    pub last_update: DateTime<Utc>,
}

impl Default for Status {
    fn default() -> Self {
        Self {
            last_update: Utc::now(),
        }
    }
}

impl Status {
    pub async fn from_db(db: &Database) -> Result<Option<Self>, mongodb::error::Error> {
        let status_n = db
            .collection::<Status>("status")
            .count_documents(doc!{})
            .await?;

        match status_n {
            0 => Ok(None),
            1 => Ok(db
                .collection::<Status>("status")
                .find_one(doc!{})
                .await?
            ),
            _ => panic!("Too many status documents in the database"),
        }
    }
}


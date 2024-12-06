use tokio::sync::Mutex;
use chrono::{DateTime, Local};
use lazy_static::lazy_static;
#[allow(unused_imports)]
use log::{debug, info, trace};
use mongodb::{bson::{doc, Bson}, options::IndexOptions, Database, IndexModel};
use futures_util::stream::{StreamExt, TryStreamExt};
use serde::{de::DeserializeOwned, Serialize};
use crate::{config::CONFIG, status::Status};

pub struct PPPDatabase {
    pub(crate) db: Database,
    status: Mutex<Option<Status>>,
}

pub trait PPPData: Serialize + DeserializeOwned + std::marker::Send + std::marker::Sync {
    const COLLECTION: &'static str;
    const ID_KEY: &'static str;
    type IdType: DeserializeOwned + std::fmt::Display + std::marker::Send + std::marker::Sync;
}

impl Default for PPPDatabase {
    fn default() -> Self {
        let db = get_client().expect("Failed to connect to MongoDB");
        Self {
            db,
            status: Mutex::new(None),
        }
    }
}

impl PPPDatabase {
    pub async fn ensure_index(&self) -> Result<(), mongodb::error::Error> {
        self.db
            .collection::<()>("transcripts")
            .create_index(IndexModel::builder()
            .keys(doc!{"data": "text"})
            .options(IndexOptions::builder().default_language("italian".to_owned()).build())
            .build()
        ).await?;
        self.db
            .collection::<()>("episodes")
            .create_index(IndexModel::builder()
                .keys(doc!{"id": 1})
                .options(IndexOptions::builder().unique(true).build())
                .build()
        ).await?;
        self.db
            .collection::<()>("users")
            .create_index(IndexModel::builder()
                .keys(doc!{"id": 1})
                .options(IndexOptions::builder().unique(true).build())
                .build()
        ).await?;
        Ok(())
    }

    pub async fn _ensure_status(&self) {
        if self.status.lock().await.is_none() {
            *self.status.lock().await = Status::from_db(&self.db).await.expect("Failed to get status from db");
        }
    }

    pub async fn _update_status(&self) {
        if self.status.lock().await.is_none() {
            *self.status.lock().await = Some(Status::default());
            self.db
                .collection::<Status>("status")
                .insert_one(self.status.lock().await.as_ref().unwrap())
                .await
                .expect("Failed to update status in db");
        } else {
            self.db
                .collection::<Status>("status")
                .replace_one(doc!{}, self.status.lock().await.as_ref().unwrap())
                .await
                .expect("Failed to update status in db");
        }
    }

    pub async fn last_modified(&self) -> Option<DateTime<Local>> {
        self._ensure_status().await;
        self.status.lock().await.as_ref().map(|s| s.last_update.clone().with_timezone(&Local))
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

    pub async fn get<T>(&self, id: T::IdType) -> Result<Option<T>, mongodb::error::Error> where T: PPPData, <T as PPPData>::IdType: Into<Bson> {
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

    pub async fn update_one_stateless<T>(&self, id: T::IdType, data: &T) -> Result<(), mongodb::error::Error> where T: PPPData, <T as PPPData>::IdType: Into<Bson> {
        self._ensure_status().await;
        self.db
            .collection::<T>(T::COLLECTION)
            .replace_one(doc!{T::ID_KEY: id}, data)
            .upsert(true)
            .await?;
        Ok(())
    }

    pub async fn update_one_stateful<T>(&self, id: T::IdType, data: &T) -> Result<(), mongodb::error::Error> where T: PPPData, <T as PPPData>::IdType: Into<Bson> {
        self._ensure_status().await;
        self.update_one_stateless(id, data).await?;
        self._update_status().await;
        Ok(())
    } 

    // pub async fn update_stateless<T>(&self, data: &[T]) -> Result<(), mongodb::error::Error> where T: PPPData {
    //     self._ensure_status().await;
    //     for d in data {
    //         self.db
    //             .collection::<T>(T::COLLECTION)
    //             .replace_one(doc!{T::ID_KEY: d.id}, d)
    //             .upsert(true)
    //             .await?;
    //     }
    //     Ok(())
    // }
}

fn get_client() -> Result<Database , Box<dyn std::error::Error>> {
    let cli = CONFIG.db.client();

    Ok(cli.database("ppp"))
}

lazy_static! {
    pub static ref DB: PPPDatabase = PPPDatabase::default();
}

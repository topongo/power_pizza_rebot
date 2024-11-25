use futures_util::TryStreamExt;
use mongodb::bson::doc;
use serde::{Serialize, Deserialize};
use tokio_stream::StreamExt;
use crate::db::{PPPData, PPPDatabase};

#[derive(Serialize, Deserialize, Debug)]
pub struct BotUser {
    pub id: i64,
    pub username: Option<String>,
    pub first_name: String,
    pub beta: bool,
    pub waitlist: bool,
    pub notified: bool,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl PPPData for BotUser {
    const ID_KEY: &'static str = "id";
    const COLLECTION: &'static str = "users";
    type IdType = i64;
}

impl From<&teloxide::types::User> for BotUser {
    fn from(u: &teloxide::types::User) -> Self {
        Self {
            id: u.id.0 as i64,
            username: u.username.clone(),
            first_name: u.first_name.clone(),
            waitlist: false,
            beta: false,
            notified: false,
            timestamp: chrono::Utc::now(),
        }
    }
}

impl BotUser {
    pub fn identify(&self) -> String {
        match self.username {
            Some(ref u) => format!("@{} ({})", u, self.id),
            None => format!("{} ({})", self.first_name, self.id),
        }
    }
}

impl PPPDatabase {
    pub async fn whitelisted(&self, id: i64) -> Result<bool, mongodb::error::Error> {
        Ok(self.db
            .collection::<()>("users")
            .count_documents(doc! { "id": id, "beta": true })
            .await? != 0)
    }
    
    pub async fn waitlist(&self) -> Result<Vec<BotUser>, mongodb::error::Error> {
        self.db
            .collection::<BotUser>("users")
            .find(doc! { "waitlist": true, "beta": false })
            .await?
            .try_collect()
            .await
    }

    pub async fn beta_list(&self) -> Result<Vec<BotUser>, mongodb::error::Error> {
        self.db
            .collection::<BotUser>("users")
            .find(doc! { "beta": true })
            .await?
            .try_collect()
            .await
    }
}


use serde::{Serialize, Deserialize};
use crate::db::PPPData;

#[derive(Serialize, Deserialize, Debug)]
pub struct BotUser {
    id: i64,
    timestamp: chrono::DateTime<chrono::Utc>,
}

impl PPPData for BotUser {
    const ID_KEY: &'static str = "id";
    const COLLECTION: &'static str = "users";
    type IdType = i64;
}

impl From<teloxide::types::User> for BotUser {
    fn from(u: teloxide::types::User) -> Self {
        Self {
            id: u.id.0 as i64,
            timestamp: chrono::Utc::now(),
        }
    }
}



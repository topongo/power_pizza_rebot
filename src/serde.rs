pub(crate) mod naive_datetime {
    use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(date: &DateTime<Utc>, s: S) -> Result<S::Ok, S::Error>
        where S: Serializer {
        s.serialize_i64(date.timestamp())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
        where D: Deserializer<'de> {
        Ok(DateTime::<Utc>::from_timestamp(i64::deserialize(deserializer)?, 0).expect("Invalid timestamp found in db"))
    }
}

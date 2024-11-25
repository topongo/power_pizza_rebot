use std::time::Duration;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use serde_with::{DurationMilliSeconds, DurationSeconds};

use crate::db::PPPData;

#[derive(Deserialize, Serialize, Debug)]
pub struct Transcript {
    pub transcription: Vec<Segment>,
}

#[derive(Deserialize, Debug)]
pub struct TranscriptAlt {
    pub segments: Vec<SegmentAlt>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Segment {
    #[serde(rename = "offsets")]
    pub timestamps: FromTo,
    pub text: String,
}

#[serde_as]
#[derive(Deserialize, Serialize, Debug)]
pub struct SegmentAlt {
    #[serde_as(as = "DurationSeconds<f64>")]
    pub start: Duration,
    #[serde_as(as = "DurationSeconds<f64>")]
    pub end: Duration,
    pub text: String,
}

#[serde_as]
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct FromTo {
    #[serde_as(as = "DurationMilliSeconds<u64>")]
    pub from: Duration,
    #[serde_as(as = "DurationMilliSeconds<u64>")]
    pub to: Duration,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Timestamp {
    pub time: FromTo,
    pub offsets: (usize, usize),
}

#[derive(Deserialize, Serialize, Debug)]
pub struct EpisodeTranscript {
    pub episode_id: u32,
    pub data: String,
    pub timestamps: Vec<Timestamp>,
}

impl PPPData for EpisodeTranscript {
    const ID_KEY: &'static str = "episode_id";
    const COLLECTION: &'static str = "transcripts";
    type IdType = u32;
}

impl From<(u32, Transcript)> for EpisodeTranscript {
    fn from(transcript: (u32, Transcript)) -> Self {
        let (episode_id, transcript) = transcript;
        let Transcript { transcription } = transcript;

        let size = transcription.iter().map(|t| t.text.len() + 1).sum::<usize>();
        let mut timestamps: Vec<Timestamp> = Vec::with_capacity(transcription.len());
        let mut data = String::with_capacity(size);
        let mut len = 0;
        for segment in transcription {
            let Segment { timestamps: ts, text: t } = segment;
            let sl = t.chars().count();
            data.push_str(&t);
            timestamps.push(Timestamp {
                time: ts,
                offsets: (len, len + sl),
            });
            len += sl;
        }
        Self {
            episode_id,
            data,
            timestamps,
        }
    }
}

impl From<TranscriptAlt> for Transcript {
    fn from(transcript: TranscriptAlt) -> Self {
        let TranscriptAlt { segments } = transcript;
        let mut transcription = Vec::with_capacity(segments.len());
        for segment in segments {
            let SegmentAlt { start, end, text } = segment;
            let timestamps = FromTo {
                from: start,
                to: end,
            };
            transcription.push(Segment { timestamps, text });
        }
        Self { transcription }
    }
}

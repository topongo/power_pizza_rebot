use std::{collections::HashSet, fs::{read_dir, read_to_string}, sync::Arc};

use log::{info, warn};
use power_pizza_bot::{db::DB, import::import_database, spreaker::Episode, transcript::{EpisodeTranscript, JobManager, Transcript}};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();

    import_database("3039391".to_owned()).await?;

    // check for missing transcripts
    let episodes = DB.get_ids::<Episode>().await.unwrap();
    let transcripts: HashSet<u32> = DB.get_ids::<EpisodeTranscript>().await?.into_iter().collect();

    // collect cached transcripts
    let cached_transcripts: HashSet<u32> = read_dir("output")
        .unwrap()
        .filter_map(|entry| {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
                match path.file_stem().unwrap().to_string_lossy().parse::<u32>() {
                    Ok(id) => Some(id),
                    Err(e) => {
                        warn!("invalid file name: {:?}: {:?}", path, e);
                        None
                    }
                }
            } else {
                None
            }
        })
        .collect();

    let audio_files: HashSet<u32> = read_dir("audio/wav")
        .unwrap()
        .filter_map(|entry| {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "wav") {
                Some(path.file_stem().unwrap().to_string_lossy().parse::<u32>().unwrap())
            } else {
                None
            }
        })
        .collect();

    let cli = Arc::new(reqwest::Client::new());
    let converter = JobManager::new(Arc::clone(&cli));
    let mut to_convert = vec![];
    let mut to_transcribe = vec![];
    let mut to_download = vec![];
    for e in episodes {
        if !transcripts.contains(&e) {
            // info!("Transcript missing for episode {}", e);
            if !audio_files.contains(&e) {
                warn!("audio file missing for {}: add to download list", e);
                to_download.push(e);
            } else if cached_transcripts.contains(&e) {
                let t = serde_json::from_str::<Transcript>(&read_to_string(format!("output/{}.json", e)).unwrap()).unwrap();
                to_convert.push((e, t));
            } else {
                info!("no cached transcript found for {}: add to transcript list", e);
                to_transcribe.push(e);
            }
        }
    }

    for e in to_download {
        converter.run_download(e);
    }
    for (e, t) in to_convert {
        converter.run_convert(e, t);
    }
    for e in to_transcribe {
        converter.run_transcribe(e);
    }

    let transcripts = converter.wait().await?;

    if !transcripts.is_empty() {
        DB.insert_stateless(&transcripts).await.unwrap();
    }

    Ok(())
}

use std::{collections::HashSet, fs::{read_dir, read_to_string}, sync::Arc};
use log::{debug, error, info, warn};
use power_pizza_bot::{config::CONFIG, db::DB, import::import_database, spreaker::Episode, transcript::{EpisodeTranscript, JobManager, Transcript}};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();

    info!("check for missing directories");
    if !CONFIG.import.check_dirs() {
        error!("missing directories");
        return Ok(());
    }

    import_database(CONFIG.import.show_id.to_string()).await?;

    // check for missing transcripts
    let episodes = DB.get_ids::<Episode>().await.unwrap();
    let transcripts: HashSet<u32> = DB.get_ids::<EpisodeTranscript>().await?.into_iter().collect();

    // collect cached transcripts
    let cached_transcripts: HashSet<u32> = read_dir(CONFIG.import.transcript_dir.clone())
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

    let audio_files: HashSet<u32> = read_dir(CONFIG.import.wav_dir.clone())
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
            if cached_transcripts.contains(&e) {
                let t = serde_json::from_str::<Transcript>(&read_to_string(format!("{}/{}.json", CONFIG.import.transcript_dir, e)).unwrap()).unwrap();
                info!("transcript cache found for {}: add to convert list", e);
                to_convert.push((e, t));
            } else if !audio_files.contains(&e) {
                warn!("transcript cache and audio file missing for {}: add to download list", e);
                to_download.push(e);
            } else {
                debug!("audio file found but no cached transcript found for {}: add to transcript list", e);
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

    converter.wait().await?;

    Ok(())
}

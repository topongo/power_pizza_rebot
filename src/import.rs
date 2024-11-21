use std::{collections::HashSet, sync::{Arc, Mutex}, time::Duration};
#[allow(unused_imports)]
use log::{info,debug,warn,error};
use crate::db::DB;
use crate::spreaker::{Episode, SimpleEpisode, SpreakerData};
use tokio_stream::StreamExt;

pub async fn import_database(show: String) -> Result<(), Box<dyn std::error::Error>> {
    info!("starting import");
    match DB.last_modified().await {
        Some(t) => {
            info!("last update: {}", t);
            let ep_ids: HashSet<u32> = DB.get_ids::<Episode>().await?.into_iter().collect();
            info!("fetching episodes");
            let mut it = SpreakerData::<SimpleEpisode>::request(
                format!("https://api.spreaker.com/v2/shows/{}/episodes", show),
                Arc::new(reqwest::Client::new()),
            );
            info!("got {} new episodes", ep_ids.len());
            let mut new_eps = vec![];
            while let Some(e) = it.next().await {
                if ep_ids.contains(&e.id) {
                    break;
                }
                let e = e.get_episode().await?;
                if !ep_ids.contains(&e.id) {
                    new_eps.push(e);
                }
            }
            info!("got {} new episodes", new_eps.len());
            if !new_eps.is_empty() {
                DB.insert_stateful::<Episode>(&new_eps).await?;
            }
        }
        None => {
            info!("no status document found, initializing database");
            let cli = Arc::new(reqwest::Client::new());
            let mut it = SpreakerData::<SimpleEpisode>::request(
                "https://api.spreaker.com/v2/shows/3039391/episodes".to_owned(),
                cli.clone(),
            );

            let mut ep_ids = vec![];
            while let Some(e) = it.next().await {
                info!("push episode {} to queue", e.id);
                // if e.id == 62738419 {
                //     continue;
                // }
                ep_ids.push(e);
            }

            let mut handles = vec![];
            let eps = Arc::new(Mutex::new(vec![]));
            for e in ep_ids {
                let eps = eps.clone();
                let h = tokio::spawn(async move {
                    info!("fetching episode {}", e.id);
                    let e = e.get_episode().await.unwrap();
                    eps.lock().unwrap().push(e);
                });
                handles.push(h);
                if handles.len() >= 10 {
                    'a: loop {
                        for ih in 0..handles.len() {
                            if handles[ih].is_finished() {
                                handles.remove(ih);
                                break 'a;
                            }
                        }
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
            for h in handles {
                h.await?;
            }

            if !eps.lock().unwrap().is_empty() {
                DB
                    .insert_stateful::<Episode>(&eps.lock().unwrap())            
                    .await?;
            }
        }
    }

    Ok(())
}

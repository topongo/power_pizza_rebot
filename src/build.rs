use log::warn;
use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize};

// {
//        "episode_id": 56245683,
//        "type": "RECORDED",
//        "title": "248: Tornado Potato (con CKibe)",
//        "duration": 3469130,
//        "explicit": false,
//        "show_id": 3039391,
//        "author_id": 10653829,
//        "image_url": "https://d3wo5wojvuv7l.cloudfront.net/t_square_limited_160/images.spreaker.com/original/e40c99acb945dbfb7145e2fc23663de2.jpg",
//        "image_original_url": "https://d3wo5wojvuv7l.cloudfront.net/images.spreaker.com/original/e40c99acb945dbfb7145e2fc23663de2.jpg",
//        "image_transformation": "square_limited_100",
//        "published_at": "2023-07-28 08:00:02",
//        "auto_published_at": null,
//        "download_enabled": true,
//        "stream_id": null,
//        "waveform_url": "https://d3770qakewhkht.cloudfront.net/episode_56245683.gz.json?v=UvNvmA",
//        "slug": "248-tornado-potato-con-ckibe",
//        "site_url": "https://www.spreaker.com/episode/248-tornado-potato-con-ckibe--56245683",
//        "download_url": "https://api.spreaker.com/v2/episodes/56245683/download.mp3",
//        "playback_url": "https://api.spreaker.com/v2/episodes/56245683/play.mp3"
//      }

#[derive(Deserialize, Debug)]
enum SpreakerResponse<T: Deserialize> {
    Success {
        items: Vec<T>,
        next_url: String,
    },
    Error {
    }
}

#[derive(Deserialize, Debug)]
struct SimpleEpisode {
    id: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    let cli = Client::new();
    
    let eps: Vec<SimpleEpisode> = cli.get("https://api.spreaker.com/v2/shows/3039391/episodes")
        .send()
        .await?
        .json()
        .await?;

    println!("{:?}", eps);
    Ok(())
}

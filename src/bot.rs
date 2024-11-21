use log::{debug, info};
use regex::Regex;
use teloxide::{adaptors::DefaultParseMode, dispatching::dialogue::GetChatId, prelude::{Requester, RequesterExt, ResponseResult}, repls::CommandReplExt, types::Message, utils::command::BotCommands, Bot};

use power_pizza_bot::db::DB;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    info!("ensuring database index");
    DB.ensure_index().await.expect("Failed to ensure index");

    let bot = Bot::from_env();
    log::info!("hi");
    Command::repl(bot, reply).await;
}

#[derive(BotCommands, Clone)]
#[command(description = "These commands are supported:")]
enum Command {
    #[command(description = "visualizza questo messaggio", rename = "help", aliases = ["start"])]
    Help,
    #[command(description = "ricerca testo semplice (titoli, descrizioni)", rename = "s", aliases = ["c", "cerca", "search"])]
    Search(String),
    #[command(description = "ricerca nel transcript di tutte le puntate", rename = "t")]
    TranscriptSearch(String),
    #[command(description = "ricerca testo del transcript una puntata, fornisci il numero della puntata e il testo", rename = "et")]
    EpisodeTranscriptSearch(String),
    #[command(description = "ricerca testo avanzata (titoli, descrizioni, transcript delle puntate)", rename = "sa")]
    AdvancedSearch(String),
}

struct PPPBot;

impl DefaultParseMode<PPPBot> {
    fn default_parse_mode(&self) -> Option<teloxide::types::ParseMode> {
        Some(teloxide::types::ParseMode::MarkdownV2)
    }
}

async fn reply(bot: Bot, msg: Message, cmd: Command) -> ResponseResult<()> {
    match cmd {
        Command::Help => bot.send_message(msg.chat.id, Command::descriptions().to_string()).await?,
        Command::Search(query) => bot.send_message(msg.chat.id, "searching...").await?,
        Command::TranscriptSearch(query) => {
            info!("received search query: {}", query);
            bot.send_message(msg.chat.id, "Searching...").await?;
            debug!("querying db");
            let results = DB.search_text(query).await.expect("Failed to search text");
            debug!("found {} results", results.len());
            bot.send_message(msg.chat.id, format!("Found episodes:\n{}", results.iter().map(|r| format!("{}: [{}](https://www.spreaker.com/episode/{})", r.episode.id, r.episode.title, r.episode.id)).collect::<Vec<_>>().join("\n"))).await?
        }
        Command::AdvancedSearch(query) => bot.send_message(msg.chat.id, "searching hard...").await?,
        Command::EpisodeTranscriptSearch(query) => {
            bot.send_message(msg.chat.id, "searching episode transcripts...").await?;
            let r = Regex::new("(?<id>[0-9]+) (?<query>.*)").unwrap();
            match r.captures(&query) {
                Some(caps) => {
                    let id = caps.name("id").unwrap();
                    let query = caps.name("query").unwrap();
                    match DB.search_transcript_offset(id.as_str().parse().unwrap(), query.as_str().to_string()).await.unwrap() {
                        Some(result) => {

                            bot.send_message(
                                msg.chat.id, 
                                format!(
                                    "Found matches:\n{}", 
                                    result.matches.iter().map(|m| format!("{:2}:{:2} - `{}`", m.time.as_secs() / 60, m.time.as_secs() % 60, m.hint)).collect::<Vec<_>>().join("\n")
                                )
                            ).await?
                        }
                        None => {
                            bot.send_message(msg.chat.id, "No matches found").await?
                        }
                    }
                                        
                }
                None => {
                    bot.send_message(msg.chat.id, "query malformata, usa questo formato `{n episodio} {query}`").await?
                }
            }
        }
    };

    Ok(())
}

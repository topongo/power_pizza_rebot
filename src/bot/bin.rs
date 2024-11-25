use std::time::Instant;

use log::{debug, error, info, trace};
use regex::Regex;
use teloxide::{prelude::Requester, repls::CommandReplExt, types::{ChatId, Message, ParseMode, User}, utils::{command::BotCommands, markdown}, Bot};
use teloxide::payloads::SendMessageSetters;
use power_pizza_bot::{bot::strings::HELP_MESSAGE, config::CONFIG};
use power_pizza_bot::{bot::{BotError, BotUser}, db::DB};

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    info!("ensuring database indexes");
    DB.ensure_index().await.expect("Failed to ensure index");

    let bot = Bot::new(CONFIG.tg.token.clone());
    log::info!("bot created, startring...");
    Command::repl(bot, reply).await;
}

fn represent_user(u: &Option<User>) -> String {
    match u {
        Some(u) => u.username.as_ref().unwrap_or(&u.first_name).to_owned(),
        None => "unknown".to_string()
    }
}

#[derive(BotCommands, Clone)]
#[command(description = "Sono supportati i seguenti messaggi:")]
enum Command {
    #[command(rename = "help", aliases = ["start"])]
    Help,
    #[command(rename = "s", aliases = ["search", "c", "cerca"])]
    Search(String),
    #[command(rename = "sa", aliases = ["searchAdvanced", "cercaAvanzato", "ca"])]
    SearchAdvanced(String),
    #[command(rename = "sae", aliases = ["searchAdvancedEpisode", "cercaAvanzatoEpisodio", "cae"])]
    SearchAdvancedEpisode(String),
}

async fn reply(bot: Bot, msg: Message, cmd: Command) -> Result<(), teloxide::RequestError> {
    match reply_inner(&bot, &msg, cmd).await {
        Ok(_) => info!("successfully replied to message {} from {}", msg.id, represent_user(&msg.from)),
        Err(e) => {
            error!("Failed to reply to message {} from {:?}: {:?}", msg.id, represent_user(&msg.from), e);
            bot.send_message(msg.chat.id, e.respond_client()).await?;
        }
    }
    Ok(())
}

async fn paginate_response(bot: &Bot, chat_id: ChatId, response: String) -> Result<(), BotError> {
    let mut message = String::with_capacity(4096);
    for chunk in response.split("\n\n") {
        if message.len() + chunk.len() + 2 > 4096 {
            bot.send_message(chat_id, &message).parse_mode(ParseMode::MarkdownV2).await?;
            message.clear();
        } else {
            message.push_str("\n\n");
        }
        message.push_str(chunk);
    }
    if !message.is_empty() {
        bot.send_message(chat_id, message).parse_mode(ParseMode::MarkdownV2).await?;
    }
    Ok(())
}

fn split_quoted_args(s: &str) -> Option<Vec<String>> {
    let mut args = vec![];
    let r = Regex::new(r#"("([^"]+)"|(\S+)")|(\S+)"#).unwrap();
    for cap in r.captures_iter(s) {
        debug!("query capture cap2 {:?}", cap.get(2));
        debug!("query capture cap4 {:?}", cap.get(4));
        debug!("query capture cap2 or cap4 {:?}", cap.get(2).or(cap.get(4)));
        let cap = match cap.get(2).or(cap.get(4)) {
            Some(c) => c,
            None => return None,
        };
        args.push(cap.as_str().to_string());
    }
    Some(args)
}

static MAX_RESULTS: usize = 50;

async fn reply_inner(bot: &Bot, msg: &Message, cmd: Command) -> Result<(), BotError> {
    let t = Instant::now();
    if let Some(u) = msg.from.clone() {
        if u.username.clone().is_some_and(|v| v != "topongo") {
            match DB.update_one_stateless(u.id.0 as i64, BotUser::from(u)).await {
                Ok(_) => bot.send_message(msg.chat.id, "Ciao, mi dispiace ma il bot è attualmente in sviluppo. Grazie per l'interesse. Riceverai una notifica quando sarà pronto.").await?,
                Err(e) => {
                    error!("Failed to update user: {:?}", e);
                    bot.send_message(msg.chat.id, "C'è stato un errore nel processare la tua richiesta").await?
                }
            };
            return Ok(());
        }
    }
    match cmd {
        Command::Help => {
            bot.send_message(msg.chat.id, &*HELP_MESSAGE)
                .parse_mode(ParseMode::MarkdownV2)
                .await?;
        }
        Command::Search(query) => {
            if query.len() < 3 {
                bot.send_message(msg.chat.id, "La query deve essere di almeno 3 caratteri").await?;
            } else {
                let results = DB.search_meta(query).await?;
                if results.len() > MAX_RESULTS {
                    bot.send_message(msg.chat.id, format!("Troppi risultati trovati ({}), per favore affina la ricerca", results.len())).await?;
                    return Ok(());
                }
                let response = results
                    .iter()
                    .map(|r| format!(
                            "{}: {}", 
                            markdown::escape(&r.episode.id.to_string()),
                            markdown::link(&format!("https://www.spreaker.com/episode/{}", r.episode.id), &markdown::escape(&r.episode.title))
                    ))
                    .collect::<Vec<_>>()
                    .join("\n");
                paginate_response(bot, msg.chat.id, response).await?;
            }
        }
        Command::SearchAdvanced(query) => {
            info!("received search query: {}", query);
            bot.send_message(msg.chat.id, "Searching...").await?;
            debug!("querying db");
            let results = DB.search_transcript_all(query).await?;
            debug!("found {} results", results.len());
            if results.len() > MAX_RESULTS {
                bot.send_message(msg.chat.id, format!("Troppi risultati trovati ({}), per favore affina la ricerca", results.len())).await?;
                return Ok(());
            }
            let response = format!(
                "{}\n{}",
                markdown::escape("Found episodes:"),
                results
                    .iter()
                    .map(|r| format!(
                            "{}: {}", 
                            markdown::escape(&r.episode.id.to_string()),
                            markdown::link(&format!("https://www.spreaker.com/episode/{}", r.episode.id), &markdown::escape(&r.episode.title))
                    ))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            paginate_response(bot, msg.chat.id, response).await?;
        }
        Command::SearchAdvancedEpisode(query) => {
            bot.send_message(msg.chat.id, "searching episode transcripts...").await?;
            let args = split_quoted_args(&query).ok_or(BotError::MalformedQuery)?;
            let id = DB.magic_episode_search(args
                .first()
                .ok_or(BotError::MalformedQuery)?.to_string()).await?;
            let query = args
                .get(1)
                .ok_or(BotError::MalformedQuery)?
                .to_string();

            info!("parsed arguments: id: {}, query: {}", id, query.as_str());
            let results = DB.search_transcript_one(id, query.as_str().to_string()).await?;
            if results.len() > MAX_RESULTS {
                bot.send_message(msg.chat.id, format!("Troppi risultati trovati ({}), per favore affina la ricerca", results.len())).await?;
                return Ok(());
            }
            if results.matches.is_empty() {
                bot.send_message(msg.chat.id, "No matches found").await?;
            } else {
                let response = format!("{}{}\n{}",
                    markdown::escape("Risultati per "),
                    markdown::link(&format!("https://www.spreaker.com/episode/{}", results.episode.id), &markdown::escape(&results.episode.title)),
                    results.matches
                        .iter()
                        .map(|m| format!(
                            "{}\n{}",
                            markdown::escape(&format!("{:02}:{:02} - {:02}:{:02}",
                                m.time.from.as_secs() / 60,
                                m.time.from.as_secs() % 60,
                                m.time.to.as_secs() / 60,
                                m.time.to.as_secs() % 60
                            )),
                            markdown::blockquote(&markdown::escape(&format!("...{}...", m.hint)))
                        ))
                        .collect::<Vec<_>>()
                        .join("\n\n")
                );
                paginate_response(&bot, msg.chat.id, response).await?;
            } 
        }
    };
    trace!("replied in {:?}", t.elapsed());

    Ok(())
}

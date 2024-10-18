use teloxide::{dispatching::dialogue::GetChatId, prelude::{Requester, ResponseResult}, repls::CommandReplExt, types::Message, utils::command::BotCommands, Bot};

#[tokio::main]
async fn main() {
    let bot = Bot::from_env();
    log::info!("hi");

    Command::repl(bot, reply).await;
}

#[derive(BotCommands, Clone)]
#[command(description = "These commands are supported:")]
enum Command {
    #[command(description = "visualizza questo messaggio", rename = "help")]
    Help,
    #[command(description = "ricerca testo semplice (titoli, descrizioni)", rename = "s")]
    Search(String),
    #[command(description = "ricerca testo avanzata (titoli, descrizioni, transcript delle puntate)", rename = "sa")]
    AdvancedSearch(String),
}

async fn reply(bot: Bot, msg: Message, cmd: Command) -> ResponseResult<()> {
    match cmd {
        Command::Help => bot.send_message(msg.chat.id, Command::descriptions().to_string()).await?,
        Command::Search(query) => bot.send_message(msg.chat.id, "searching...").await?,
        Command::AdvancedSearch(query) => bot.send_message(msg.chat.id, "searching hard...").await?,
    };

    Ok(())
}

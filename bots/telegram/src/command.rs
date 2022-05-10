use color_eyre::Result;
use teloxide::{prelude::*, utils::command::BotCommands};

#[derive(Debug, Clone, BotCommands)]
#[command(rename = "lowercase", description = "These commands are supported")]
pub enum Command {
    #[command(description = "Display this text.")]
    Help,
    #[command(description = "Register in the system.")]
    Register,
    #[command(description = "Set preferences.")]
    Setting,
    #[command(description = "Delete your account.")]
    DeleteAccount,
}

/// Answer to command
pub(crate) async fn answer(bot: AutoSend<Bot>, message: Message, command: Command) -> Result<()> {
    match command {
        Command::Help => {
            bot.send_message(message.chat.id, Command::descriptions().to_string())
                .await?
        }
        Command::Register => todo!(),
        Command::Setting => todo!(),
        Command::DeleteAccount => todo!(),
    };

    Ok(())
}

use crate::command::args::difficulty::DifficultyArgumentConsumer;
use crate::command::args::{Arg, GetCloned};
use crate::command::dispatcher::CommandError::InvalidConsumption;
use crate::command::tree::builder::argument;
use crate::command::{
    CommandError, CommandExecutor, CommandSender, args::ConsumedArgs, tree::CommandTree,
};

use pumpkin_util::text::TextComponent;

const NAMES: [&str; 1] = ["difficulty"];

const DESCRIPTION: &str = "Change the difficulty of the world.";

pub const ARG_DIFFICULTY: &str = "difficulty";
struct DifficultyExecutor;


impl CommandExecutor for DifficultyExecutor {
    fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        server: &crate::server::Server,
        args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        let Some(Arg::Difficulty(difficulty)) = args.get_cloned(&ARG_DIFFICULTY) else {
            return Err(InvalidConsumption(Some(ARG_DIFFICULTY.into())));
        };

        let difficulty_string = format!("{difficulty:?}").to_lowercase();
        let translation_key = format!("options.difficulty.{difficulty_string}");

        {
            let level_info = server.level_info.read();

            if level_info.difficulty == difficulty {
                sender.send_message(TextComponent::translate(
                    "commands.difficulty.failure",
                    [TextComponent::translate(translation_key, [])],
                ));
                return Ok(());
            }
        }

        server.set_difficulty(difficulty, Some(true));

        sender.send_message(TextComponent::translate(
            "commands.difficulty.success",
            [TextComponent::translate(translation_key, [])],
        ));

        Ok(())
    }
}

#[must_use]
pub fn init_command_tree() -> CommandTree {
    CommandTree::new(NAMES, DESCRIPTION)
        .then(argument(ARG_DIFFICULTY, DifficultyArgumentConsumer).execute(DifficultyExecutor))
}

use crate::{
    command::{
        CommandError, CommandExecutor, CommandSender,
        args::{Arg, ConsumedArgs, simple::SimpleArgConsumer},
        tree::CommandTree,
        tree::builder::argument,
    },
    data::{banned_ip_data::BANNED_IP_LIST, banned_player_data::BANNED_PLAYER_LIST},
};
use CommandError::InvalidConsumption;

use pumpkin_util::text::TextComponent;

const NAMES: [&str; 1] = ["banlist"];
const DESCRIPTION: &str = "shows the banlist";

const ARG_LIST_TYPE: &str = "ips|players";

struct ListExecutor;


impl CommandExecutor for ListExecutor {
    fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        _server: &crate::server::Server,
        args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        let Some(Arg::Simple(list_type)) = args.get(&ARG_LIST_TYPE) else {
            return Err(InvalidConsumption(Some(ARG_LIST_TYPE.into())));
        };

        match *list_type {
            "ips" => {
                let lock = &BANNED_IP_LIST.read();
                let entries = lock
                    .banned_ips
                    .iter()
                    .map(|entry| {
                        (
                            entry.ip.to_string(),
                            entry.source.clone(),
                            entry.reason.clone(),
                        )
                    })
                    .collect();

                handle_banlist(entries, sender);
            }
            "players" => {
                let lock = &BANNED_PLAYER_LIST.read();
                let entries = lock
                    .banned_players
                    .iter()
                    .map(|entry| {
                        (
                            entry.name.clone(),
                            entry.source.clone(),
                            entry.reason.clone(),
                        )
                    })
                    .collect();

                handle_banlist(entries, sender);
            }
            _ => {
                return Err(CommandError::CommandFailed(Box::new(TextComponent::text(
                    "Incorrect argument for command".to_string(),
                ))));
            }
        }

        Ok(())
    }
}

struct ListAllExecutor;


impl CommandExecutor for ListAllExecutor {
    fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        _server: &crate::server::Server,
        _args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        let mut entries = Vec::new();
        for entry in &BANNED_PLAYER_LIST.read().banned_players {
            entries.push((
                entry.name.clone(),
                entry.source.clone(),
                entry.reason.clone(),
            ));
        }

        for entry in &BANNED_IP_LIST.read().banned_ips {
            entries.push((
                entry.ip.to_string(),
                entry.source.clone(),
                entry.reason.clone(),
            ));
        }

        handle_banlist(entries, sender);
        Ok(())
    }
}

/// `Vec<(name, source, reason)>`
fn handle_banlist(list: Vec<(String, String, String)>, sender: &CommandSender) {
    if list.is_empty() {
        sender.send_message(TextComponent::translate("commands.banlist.none", []));
        return;
    }

    sender.send_message(TextComponent::translate(
        "commands.banlist.list",
        [TextComponent::text(list.len().to_string())],
    ));

    for (name, source, reason) in list {
        sender.send_message(TextComponent::translate(
            "commands.banlist.entry",
            [
                TextComponent::text(name),
                TextComponent::text(source),
                TextComponent::text(reason),
            ],
        ));
    }
}

pub fn init_command_tree() -> CommandTree {
    CommandTree::new(NAMES, DESCRIPTION)
        .execute(ListAllExecutor)
        .then(argument(ARG_LIST_TYPE, SimpleArgConsumer).execute(ListExecutor))
}

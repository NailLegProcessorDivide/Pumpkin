use std::sync::atomic::Ordering;

use pumpkin_config::{BASIC_CONFIG, whitelist::WhitelistEntry};
use pumpkin_util::text::TextComponent;

use crate::entity::EntityBase;
use crate::{
    command::{
        CommandExecutor, CommandSender,
        args::{Arg, ConsumedArgs, players::PlayersArgumentConsumer},
        dispatcher::CommandError,
        tree::{
            CommandTree,
            builder::{argument, literal},
        },
    },
    data::{
        LoadJSONConfiguration, SaveJSONConfiguration,
        whitelist_data::{WHITELIST_CONFIG, WhitelistConfig},
    },
    net::DisconnectReason,
    server::Server,
};

const NAMES: [&str; 1] = ["whitelist"];
const DESCRIPTION: &str = "Manage server whitelists.";
const ARG_TARGETS: &str = "targets";

fn kick_non_whitelisted_players(server: &Server) {
    let whitelist = WHITELIST_CONFIG.read();
    if BASIC_CONFIG.enforce_whitelist && server.white_list.load(Ordering::Relaxed) {
        for player in server.get_all_players() {
            if whitelist.is_whitelisted(&player.gameprofile) {
                continue;
            }
            player.kick(
                DisconnectReason::Kicked,
                TextComponent::translate("multiplayer.disconnect.not_whitelisted", &[]),
            );
        }
    }
}

struct OnExecutor;


impl CommandExecutor for OnExecutor {
    fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        server: &Server,
        _args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        let previous = server.white_list.swap(true, Ordering::Relaxed);
        if previous {
            sender.send_message(TextComponent::translate(
                "commands.whitelist.alreadyOn",
                &[],
            ));
        } else {
            kick_non_whitelisted_players(server);
            sender.send_message(TextComponent::translate("commands.whitelist.enabled", &[]));
        }
        Ok(())
    }
}

struct OffExecutor;


impl CommandExecutor for OffExecutor {
    fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        server: &Server,
        _args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        let previous = server.white_list.swap(false, Ordering::Relaxed);
        if previous {
            sender.send_message(TextComponent::translate("commands.whitelist.disabled", &[]));
        } else {
            sender.send_message(TextComponent::translate(
                "commands.whitelist.alreadyOff",
                &[],
            ));
        }
        Ok(())
    }
}

struct ListExecutor;


impl CommandExecutor for ListExecutor {
    fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        _server: &Server,
        _args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        let whitelist = &WHITELIST_CONFIG.read().whitelist;
        if whitelist.is_empty() {
            sender.send_message(TextComponent::translate("commands.whitelist.none", []));
            return Ok(());
        }

        let names = whitelist
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<&str>>()
            .join(", ");

        sender.send_message(TextComponent::translate(
            "commands.whitelist.list",
            [
                TextComponent::text(whitelist.len().to_string()),
                TextComponent::text(names),
            ],
        ));

        Ok(())
    }
}

struct ReloadExecutor;


impl CommandExecutor for ReloadExecutor {
    fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        server: &Server,
        _args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        *WHITELIST_CONFIG.write() = WhitelistConfig::load();
        kick_non_whitelisted_players(server);
        sender.send_message(TextComponent::translate("commands.whitelist.reloaded", &[]));
        Ok(())
    }
}

pub struct AddExecutor;


impl CommandExecutor for AddExecutor {
    fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        _server: &Server,
        args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        let Some(Arg::Players(targets)) = args.get(&ARG_TARGETS) else {
            return Err(CommandError::InvalidConsumption(Some(ARG_TARGETS.into())));
        };

        let mut whitelist = WHITELIST_CONFIG.write();
        for player in targets {
            let profile = &player.gameprofile;
            if whitelist.is_whitelisted(profile) {
                sender.send_message(TextComponent::translate(
                    "commands.whitelist.add.failed",
                    &[],
                ));
                continue;
            }
            whitelist
                .whitelist
                .push(WhitelistEntry::new(profile.id, profile.name.clone()));
            sender.send_message(TextComponent::translate(
                "commands.whitelist.add.success",
                [TextComponent::text(profile.name.clone())],
            ));
        }

        whitelist.save();
        Ok(())
    }
}

pub struct RemoveExecutor;


impl CommandExecutor for RemoveExecutor {
    fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        server: &Server,
        args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError> {
        let Some(Arg::Players(targets)) = args.get(&ARG_TARGETS) else {
            return Err(CommandError::InvalidConsumption(Some(ARG_TARGETS.into())));
        };

        let mut whitelist = WHITELIST_CONFIG.write();
        for player in targets {
            let i = whitelist
                .whitelist
                .iter()
                .position(|entry| entry.uuid == player.gameprofile.id);

            match i {
                Some(i) => {
                    whitelist.whitelist.remove(i);
                    sender.send_message(TextComponent::translate(
                        "commands.whitelist.remove.success",
                        [player.get_display_name()],
                    ));
                }
                None => {
                    sender.send_message(TextComponent::translate(
                        "commands.whitelist.remove.failed",
                        [],
                    ));
                }
            }
        }

        whitelist.save();
        drop(whitelist);

        kick_non_whitelisted_players(server);
        Ok(())
    }
}

pub fn init_command_tree() -> CommandTree {
    CommandTree::new(NAMES, DESCRIPTION)
        .then(literal("on").execute(OnExecutor))
        .then(literal("off").execute(OffExecutor))
        .then(literal("list").execute(ListExecutor))
        .then(literal("reload").execute(ReloadExecutor))
        .then(
            literal("add")
                .then(argument(ARG_TARGETS, PlayersArgumentConsumer).execute(AddExecutor)),
        )
        .then(
            literal("remove")
                .then(argument(ARG_TARGETS, PlayersArgumentConsumer).execute(RemoveExecutor)),
        )
}

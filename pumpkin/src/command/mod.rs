use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use crate::entity::player::Player;
use crate::server::Server;
use crate::world::World;
use args::ConsumedArgs;

use dispatcher::CommandError;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_util::permission::PermissionLvl;
use pumpkin_util::text::TextComponent;
use pumpkin_util::translation::Locale;
use tokio::sync::mpsc;

pub mod args;
pub mod client_suggestions;
pub mod commands;
pub mod dispatcher;
pub mod tree;

pub enum CommandSender {
    Rcon(mpsc::Sender<String>),
    Console,
    Player(Arc<Player>),
}

impl fmt::Display for CommandSender {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Console => "Server",
                Self::Rcon(_) => "Rcon",
                Self::Player(p) => &p.gameprofile.name,
            }
        )
    }
}

impl CommandSender {
    pub fn send_message(&self, text: TextComponent) {
        match self {
            Self::Console => log::info!("{}", text.to_pretty_console()),
            Self::Player(c) => c.send_system_message(&text),
            Self::Rcon(s) => s
                .try_send(text.to_pretty_console())
                .expect("Error sending message to RCON"),
        }
    }

    #[must_use]
    pub const fn is_player(&self) -> bool {
        matches!(self, Self::Player(_))
    }

    #[must_use]
    pub const fn is_console(&self) -> bool {
        matches!(self, Self::Console)
    }
    #[must_use]
    pub fn as_player(&self) -> Option<Arc<Player>> {
        match self {
            Self::Player(player) => Some(player.clone()),
            _ => None,
        }
    }

    /// prefer using `has_permission_lvl(lvl)`
    #[must_use]
    pub fn permission_lvl(&self) -> PermissionLvl {
        match self {
            Self::Console | Self::Rcon(_) => PermissionLvl::Four,
            Self::Player(p) => p.permission_lvl.load(),
        }
    }

    #[must_use]
    pub fn has_permission_lvl(&self, lvl: PermissionLvl) -> bool {
        match self {
            Self::Console | Self::Rcon(_) => true,
            Self::Player(p) => p.permission_lvl.load().ge(&lvl),
        }
    }

    /// Check if the sender has a specific permission
    pub fn has_permission(&self, node: &str) -> bool {
        match self {
            Self::Console | Self::Rcon(_) => true, // Console and RCON always have all permissions
            Self::Player(p) => p.has_permission(node),
        }
    }

    #[must_use]
    pub fn position(&self) -> Option<Vector3<f64>> {
        match self {
            Self::Console | Self::Rcon(..) => None,
            Self::Player(p) => Some(p.living_entity.entity.pos.load()),
        }
    }

    #[must_use]
    pub fn world(&self) -> Option<Arc<World>> {
        match self {
            // TODO: maybe return first world when console
            Self::Console | Self::Rcon(..) => None,
            Self::Player(p) => Some(p.living_entity.entity.world.clone()),
        }
    }

    pub fn get_locale(&self) -> Locale {
        match self {
            Self::Console | Self::Rcon(..) => Locale::EnUs, // Default locale for console and RCON
            Self::Player(player) => {
                Locale::from_str(&player.config.read().locale).unwrap_or(Locale::EnUs)
            }
        }
    }
}

pub trait CommandExecutor: Sync {
    fn execute<'a>(
        &self,
        sender: &mut CommandSender,
        server: &Server,
        args: &ConsumedArgs<'a>,
    ) -> Result<(), CommandError>;
}

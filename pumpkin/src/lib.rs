// Not warn event sending macros
#![allow(unused_labels)]

use crate::logging::{GzipRollingLogger, ReadlineLogWrapper};
use crate::net::DisconnectReason;
use crate::net::net_thread::{ConnectionInfo, NetworkThreadHandle};
use crate::server::{Server, ticker::Ticker};
use log::{Level, LevelFilter};
use net::authentication::fetch_mojang_public_keys;
use parking_lot::RwLock;
use plugin::PluginManager;
use pumpkin_config::{BASIC_CONFIG, advanced_config};
use pumpkin_util::permission::{PermissionManager, PermissionRegistry};
use pumpkin_util::text::TextComponent;
use rustyline_async::Readline;
use simplelog::SharedLogger;
use std::io::{IsTerminal, stdin};
use std::num::NonZeroU32;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Notify;
use tokio_util::task::TaskTracker;

pub mod block;
pub mod command;
pub mod data;
pub mod entity;
pub mod error;
pub mod item;
pub mod logging;
pub mod net;
pub mod plugin;
pub mod server;
pub mod world;

#[cfg(feature = "dhat-heap")]
pub static HEAP_PROFILER: LazyLock<Mutex<Option<dhat::Profiler>>> =
    LazyLock::new(|| Mutex::new(None));

pub static PLUGIN_MANAGER: LazyLock<Arc<PluginManager>> =
    LazyLock::new(|| Arc::new(PluginManager::new()));

pub static PERMISSION_REGISTRY: LazyLock<Arc<RwLock<PermissionRegistry>>> =
    LazyLock::new(|| Arc::new(RwLock::new(PermissionRegistry::new())));

pub static PERMISSION_MANAGER: LazyLock<Arc<RwLock<PermissionManager>>> = LazyLock::new(|| {
    Arc::new(RwLock::new(PermissionManager::new(
        PERMISSION_REGISTRY.clone(),
    )))
});

const TWENTY_HZ: Option<NonZeroU32> = NonZeroU32::new(20);

pub static LOGGER_IMPL: LazyLock<Option<(ReadlineLogWrapper, LevelFilter)>> = LazyLock::new(|| {
    if advanced_config().logging.enabled {
        let mut config = simplelog::ConfigBuilder::new();

        if advanced_config().logging.timestamp {
            config.set_time_format_custom(time::macros::format_description!(
                "[year]-[month]-[day] [hour]:[minute]:[second]"
            ));
            config.set_time_level(LevelFilter::Error);
            let _ = config.set_time_offset_to_local();
        } else {
            config.set_time_level(LevelFilter::Off);
        }

        if !advanced_config().logging.color {
            for level in Level::iter() {
                config.set_level_color(level, None);
            }
        } else {
            // We are technically logging to a file-like object.
            config.set_write_log_enable_colors(true);
        }

        if !advanced_config().logging.threads {
            config.set_thread_level(LevelFilter::Off);
        } else {
            config.set_thread_level(LevelFilter::Info);
        }

        let level = std::env::var("RUST_LOG")
            .ok()
            .as_deref()
            .map(LevelFilter::from_str)
            .and_then(Result::ok)
            .unwrap_or(LevelFilter::Info);

        let file_logger: Option<Box<dyn SharedLogger + 'static>> =
            if advanced_config().logging.file.is_empty() {
                None
            } else {
                Some(
                    GzipRollingLogger::new(
                        level,
                        {
                            let mut config = config.clone();
                            for level in Level::iter() {
                                config.set_level_color(level, None);
                            }
                            config.build()
                        },
                        advanced_config().logging.file.clone(),
                    )
                    .expect("Failed to initialize file logger.")
                        as Box<dyn SharedLogger>,
                )
            };

        if advanced_config().commands.use_tty && stdin().is_terminal() {
            match Readline::new("$ ".to_owned()) {
                Ok((rl, stdout)) => {
                    let logger = simplelog::WriteLogger::new(level, config.build(), stdout);
                    Some((
                        ReadlineLogWrapper::new(logger, file_logger, Some(rl)),
                        level,
                    ))
                }
                Err(e) => {
                    log::warn!(
                        "Failed to initialize console input ({e}); falling back to simple logger"
                    );
                    let logger = simplelog::SimpleLogger::new(level, config.build());
                    Some((ReadlineLogWrapper::new(logger, file_logger, None), level))
                }
            }
        } else {
            let logger = simplelog::SimpleLogger::new(level, config.build());
            Some((ReadlineLogWrapper::new(logger, file_logger, None), level))
        }
    } else {
        None
    }
});

#[macro_export]
macro_rules! init_log {
    () => {
        if let Some((logger_impl, level)) = &*pumpkin::LOGGER_IMPL {
            log::set_logger(logger_impl).unwrap();
            log::set_max_level(*level);
        }
    };
}

pub static SHOULD_STOP: AtomicBool = AtomicBool::new(false);
pub static STOP_INTERRUPT: LazyLock<Notify> = LazyLock::new(Notify::new);

pub fn stop_server() {
    SHOULD_STOP.store(true, Ordering::Relaxed);
    STOP_INTERRUPT.notify_waiters();
}

pub struct PumpkinServer {
    pub server: Arc<Server>,
}

impl PumpkinServer {
    pub fn new() -> Self {
        let server = Server::new();

        if BASIC_CONFIG.allow_chat_reports {
            let mojang_public_keys = fetch_mojang_public_keys().unwrap();
            *server.mojang_public_keys.lock() = mojang_public_keys;
        }

        Self {
            server: server.clone(),
        }
    }

    pub async fn init_plugins(&self) {
        PLUGIN_MANAGER.set_self_ref(PLUGIN_MANAGER.clone());
        PLUGIN_MANAGER.set_server(self.server.clone());
        if let Err(err) = PLUGIN_MANAGER.load_plugins() {
            log::error!("{err}");
        };
    }

    pub async fn unload_plugins(&self) {
        if let Err(err) = PLUGIN_MANAGER.unload_all_plugins() {
            log::error!("Error unloading plugins: {err}");
        } else {
            log::info!("All plugins unloaded successfully");
        }
    }

    pub fn start(&self) {
        let net_thread = NetworkThreadHandle::start_net_thread(ConnectionInfo {});
        let tasks = Arc::new(TaskTracker::new());

        let mut ticker = Ticker::new(TWENTY_HZ);
        ticker.run(|| self.server.tick());

        log::info!("Stopped accepting incoming connections");

        if let Err(e) = self
            .server
            .player_data_storage
            .save_all_players(&self.server)
        {
            log::error!("Error saving all players during shutdown: {e}");
        }

        let kick_message = TextComponent::text("Server stopped");
        for player in self.server.get_all_players() {
            player.kick(DisconnectReason::Shutdown, kick_message.clone());
        }

        log::info!("Ending player tasks");

        tasks.close();
        tasks.wait();

        self.unload_plugins();

        log::info!("Starting save.");

        self.server.shutdown();

        log::info!("Completed save!");

        // Explicitly drop the line reader to return the terminal to the original state.
        if let Some((wrapper, _)) = &*LOGGER_IMPL
            && let Some(rl) = wrapper.take_readline()
        {
            let _ = rl;
        }
    }
}

fn scrub_address(ip: &str) -> String {
    ip.chars()
        .map(|ch| if ch == '.' || ch == ':' { ch } else { 'x' })
        .collect()
}

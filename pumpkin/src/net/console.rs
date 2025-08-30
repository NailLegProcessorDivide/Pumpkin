use std::{io::stdin, sync::atomic::Ordering};

use rustyline_async::{Readline, ReadlineEvent};
use tokio::{
    select,
    sync::mpsc::{Sender, channel},
};

use crate::{LOGGER_IMPL, SHOULD_STOP, STOP_INTERRUPT, net::net_thread::NetResponse, stop_server};

pub async fn setup_stdin_console(server: Sender<NetResponse>) {
    tokio::spawn(async move {
        while !SHOULD_STOP.load(Ordering::Relaxed) {
            let mut line = String::new();
            if let Ok(size) = stdin().read_line(&mut line) {
                // if no bytes were read, we may have hit EOF
                if size == 0 {
                    break;
                }
            } else {
                break;
            };
            if line.is_empty() || line.as_bytes()[line.len() - 1] != b'\n' {
                log::warn!("Console command was not terminated with a newline");
            }
            let (tx, mut rx) = channel(16);
            server
                .send(NetResponse::Command(line.trim().to_string(), tx))
                .await
                .expect("Failed to send command to server");
        }
    });
}

pub async fn setup_console(rl: Readline, server: Sender<NetResponse>) {
    // This needs to be async, or it will hog a thread.
    tokio::spawn(async move {
        let mut rl = rl;
        while !SHOULD_STOP.load(Ordering::Relaxed) {
            let t1 = rl.readline();
            let t2 = STOP_INTERRUPT.notified();

            let result = select! {
                line = t1 => Some(line),
                () = t2 => None,
            };

            let Some(result) = result else { break };

            match result {
                Ok(ReadlineEvent::Line(line)) => {
                    let (tx, mut rx) = channel(16);
                    server.send(NetResponse::Command(line.clone(), tx));
                    rl.add_history_entry(line).unwrap();
                }
                Ok(ReadlineEvent::Interrupted) => {
                    stop_server();
                    break;
                }
                err => {
                    log::error!("Console command loop failed!");
                    log::error!("{err:?}");
                    break;
                }
            }
        }
        if let Some((wrapper, _)) = &*LOGGER_IMPL {
            wrapper.return_readline(rl);
        }

        log::debug!("Stopped console commands task");
    });
}

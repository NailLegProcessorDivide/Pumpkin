use std::{
    collections::HashMap,
    ffi::{CString, NulError},
    net::SocketAddr,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

use parking_lot::RwLock;
use pumpkin_protocol::query::{
    CBasicStatus, CFullStatus, CHandshake, PacketType, RawQueryPacket, SHandshake, SStatusRequest,
};
use rand::Rng;
use tokio::{
    net::UdpSocket,
    sync::{mpsc::Sender, oneshot},
    time,
};

use crate::{SHOULD_STOP, STOP_INTERRUPT, net::net_thread::NetResponse};

pub enum QueryReq {
    Basic(oneshot::Sender<BasicServerData>),
    Full(oneshot::Sender<FullServerData>),
}

pub struct BasicServerData {
    motd: CString,
    map: CString,
    num_players: usize,
    max_players: usize,
}

pub struct FullServerData {
    basic: BasicServerData,
    version: CString,
    plugins: CString,
    players: Vec<CString>,
}

pub async fn start_query_handler(server: Sender<NetResponse>, query_addr: SocketAddr) {
    let socket = Arc::new(
        UdpSocket::bind(query_addr)
            .await
            .expect("Unable to bind to address"),
    );

    // Challenge tokens are bound to the IP address and port
    let valid_challenge_tokens = Arc::new(RwLock::new(HashMap::new()));
    let valid_challenge_tokens_clone = valid_challenge_tokens.clone();
    // All challenge tokens ever created are expired every 30 seconds
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(30));

        loop {
            interval.tick().await;
            valid_challenge_tokens_clone.write().clear();
        }
    });

    log::info!(
        "Server query running on port {}",
        socket
            .local_addr()
            .expect("Unable to find running address!")
            .port()
    );

    while !SHOULD_STOP.load(Ordering::Relaxed) {
        let socket = socket.clone();
        let valid_challenge_tokens = valid_challenge_tokens.clone();
        let server = server.clone();
        let mut buf = vec![0; 1024];

        let recv_result = tokio::select! {
            result = socket.recv_from(&mut buf) => Some(result),
            () = STOP_INTERRUPT.notified() => None,
        };

        let Some(Ok((_, addr))) = recv_result else {
            break;
        };

        tokio::spawn(async move {
            if let Err(err) = handle_packet(
                buf,
                valid_challenge_tokens,
                socket,
                addr,
                query_addr,
                server,
            )
            .await
            {
                log::error!("Interior 0 bytes found! Cannot encode query response! {err}");
            }
        });
    }
}

// Errors of packets that don't meet the format aren't returned since we won't handle them anyway
// The only errors that are thrown are because of a null terminator in a CString
// since those errors need to be corrected by server owner
#[inline]
async fn handle_packet(
    buf: Vec<u8>,
    clients: Arc<RwLock<HashMap<i32, SocketAddr>>>,
    socket: Arc<UdpSocket>,
    addr: SocketAddr,
    bound_addr: SocketAddr,
    info_requester: Sender<NetResponse>,
) -> Result<(), NulError> {
    if let Ok(mut raw_packet) = RawQueryPacket::decode(buf) {
        match raw_packet.packet_type {
            PacketType::Handshake => {
                if let Ok(packet) = SHandshake::decode(&mut raw_packet) {
                    let challenge_token = rand::rng().random_range(1..=i32::MAX);
                    let response = CHandshake {
                        session_id: packet.session_id,
                        challenge_token,
                    };

                    // Ignore all errors since we don't want the query handler to crash
                    // Protocol also ignores all errors and just doesn't respond
                    let _ = socket.send_to(response.encode().as_slice(), addr).await;

                    clients.write().insert(challenge_token, addr);
                }
            }
            PacketType::Status => {
                let Ok(packet) = SStatusRequest::decode(&mut raw_packet) else {
                    // silent error
                    return Ok(());
                };
                if clients
                    .read()
                    .get(&packet.challenge_token)
                    .is_some_and(|token_bound_ip: &SocketAddr| token_bound_ip == &addr)
                {
                    handle_status(&packet, info_requester, addr, bound_addr, socket);
                }
            }
        }
    }
    Ok(())
}

async fn handle_status(
    packet: &SStatusRequest,
    info_requester: Sender<NetResponse>,
    addr: SocketAddr,
    bound_addr: SocketAddr,
    socket: Arc<UdpSocket>,
) -> Result<(), NulError> {
    let session_id = packet.session_id;
    if packet.is_full_request {
        tokio::spawn(async move {
            let (tx, rx) = oneshot::channel();
            info_requester.send(NetResponse::QueryFull(tx)).await;
            if let Ok(resp) = rx.await {
                let Ok(host_ip) = CString::new(bound_addr.ip().to_string()) else {
                    return;
                };
                let response = CFullStatus {
                    session_id,
                    hostname: resp.basic.motd,
                    version: resp.version,
                    plugins: resp.plugins,
                    map: resp.basic.map, // TODO: Get actual world name
                    num_players: resp.basic.num_players,
                    max_players: resp.basic.max_players,
                    host_port: bound_addr.port(),
                    host_ip,
                    players: resp.players,
                };
                let _ = socket.send_to(response.encode().as_slice(), addr).await;
            }
        });
    } else {
        tokio::spawn(async move {
            let (tx, rx) = oneshot::channel();
            info_requester.send(NetResponse::QueryBasic(tx)).await;
            if let Ok(resp) = rx.await {
                let Ok(host_ip) = CString::new(bound_addr.ip().to_string()) else {
                    return;
                };
                let response = CBasicStatus {
                    session_id,
                    motd: resp.motd,
                    map: resp.map,
                    num_players: resp.num_players,
                    max_players: resp.max_players,
                    host_port: bound_addr.port(),
                    host_ip,
                };
                let _ = socket.send_to(response.encode().as_slice(), addr).await;
            }
        });
    }
    Ok(())
}

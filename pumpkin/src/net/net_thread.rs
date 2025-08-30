use std::{
    io::{Cursor, Error},
    net::SocketAddr,
    sync::Arc,
    thread::{self, JoinHandle},
};

use dashmap::DashMap;
use parking_lot::RwLock;
use pumpkin_config::{BASIC_CONFIG, advanced_config};
use pumpkin_world::chunk::ChunkData;
use tokio::{
    net::{TcpListener, TcpStream, UdpSocket},
    runtime::Builder,
    select,
    sync::{
        mpsc::{
            Receiver, Sender, UnboundedReceiver, UnboundedSender, channel, error::SendError,
            unbounded_channel,
        },
        oneshot,
    },
};
use tokio_util::task::TaskTracker;

use crate::{
    LOGGER_IMPL, STOP_INTERRUPT,
    net::{
        bedrock::BedrockClient,
        console::{setup_console, setup_stdin_console},
        java::JavaClient,
        key_store::KeyStore,
        lan_broadcast,
        query::{self, BasicServerData, FullServerData},
        rcon::RCONServer,
    },
    scrub_address,
};

// Capacity of channels between server and networking module
const CHANNEL_DEPTH: usize = 128;

pub enum ServerClientEvent {
    SendChunks(Box<[Arc<RwLock<ChunkData>>]>),
}

pub enum ClientServerEvent {
    StatusRequest,
    CanPlayerJoin(String, uuid::Uuid, oneshot::Sender<bool>),
}

pub struct ClientHandle {
    sender: UnboundedSender<ClientServerEvent>,
    receiver: UnboundedReceiver<ServerClientEvent>,
}

pub struct ServerHandle {
    sender: UnboundedSender<ServerClientEvent>,
    receiver: UnboundedReceiver<ClientServerEvent>,
}

pub enum NetEvent {}

// Messages from the network handler to the game
pub enum NetResponse {
    Command(String, Sender<String>),
    QueryBasic(oneshot::Sender<BasicServerData>),
    QueryFull(oneshot::Sender<FullServerData>),
    AddPlayer(ServerHandle),
}

pub struct NetworkThreadHandle {
    handle: JoinHandle<()>,
    net_server_hand: Sender<NetEvent>,
    game_receiver: Receiver<NetResponse>,
}

pub struct ConnectionInfo {}

impl NetworkThreadHandle {
    pub fn start_net_thread(state: ConnectionInfo) -> Self {
        let (net_server_hand, net_receiver) = channel(CHANNEL_DEPTH);
        let (game_sender, game_receiver) = channel(CHANNEL_DEPTH);

        Self {
            handle: thread::spawn(move || network_thread_rt(state, net_receiver, game_sender)),
            net_server_hand,
            game_receiver,
        }
    }
}

fn network_thread_rt(
    info: ConnectionInfo,
    receiver: Receiver<NetEvent>,
    server_hand: Sender<NetResponse>,
) {
    let rt = Builder::new_multi_thread()
        .enable_all()
        .thread_name("pumpkin-net")
        .build()
        .expect("error constructing tokio runtime for io");

    rt.block_on(NetworkThread::new(info, receiver, server_hand).run());
}

struct NetworkThread {
    info: ConnectionInfo,
    receiver: Receiver<NetEvent>,
    server_hand: Sender<NetResponse>,
    bedrock_clients: DashMap<SocketAddr, ClientHandle>,
    master_client_id_counter: u64,
    tracker: TaskTracker,

    /// Handles cryptographic keys for secure communication.
    key_store: Arc<KeyStore>,
}

impl NetworkThread {
    fn new(
        info: ConnectionInfo,
        receiver: Receiver<NetEvent>,
        server_hand: Sender<NetResponse>,
    ) -> Self {
        Self {
            info,
            receiver,
            server_hand,
            bedrock_clients: DashMap::new(),
            master_client_id_counter: 0,
            tracker: TaskTracker::new(),
            key_store: Arc::new(KeyStore::new()),
        }
    }

    async fn run(&mut self) {
        let mut java_listener = TcpListener::bind(BASIC_CONFIG.java_edition_address)
            .await
            .expect("Failed to start `TcpListener`");
        let mut bedrock_socket = UdpSocket::bind(BASIC_CONFIG.bedrock_edition_address)
            .await
            .expect("Failed to bind UDP (bedrock) Socket");

        let rcon = advanced_config().networking.rcon.clone();

        if rcon.enabled {
            log::warn!(
                "RCON is enabled, but it's highly insecure as it transmits passwords and commands in plain text. This makes it vulnerable to interception and exploitation by anyone on the network"
            );
            let server_hand = self.server_hand.clone();
            self.tracker.spawn(async move {
                RCONServer::run(&rcon, server_hand).await.unwrap();
            });
        }

        if advanced_config().commands.use_console
            && let Some((wrapper, _)) = &*LOGGER_IMPL
        {
            if let Some(rl) = wrapper.take_readline() {
                setup_console(rl, self.server_hand.clone()).await;
            } else {
                if advanced_config().commands.use_tty {
                    log::warn!(
                        "The input is not a TTY; falling back to simple logger and ignoring `use_tty` setting"
                    );
                }
                setup_stdin_console(self.server_hand.clone()).await;
            }
        }

        if advanced_config().networking.query.enabled {
            log::info!("Query protocol is enabled. Starting...");
            self.tracker.spawn(query::start_query_handler(
                self.server_hand.clone(),
                advanced_config().networking.query.address,
            ));
        }

        let addr = java_listener
            .local_addr()
            .expect("Unable to get the address of the server!");

        if advanced_config().networking.lan_broadcast.enabled {
            log::info!("LAN broadcast is enabled. Starting...");
            self.tracker.spawn(lan_broadcast::start_lan_broadcast(addr));
        }

        while self
            .unified_listener_task(&mut java_listener, &mut bedrock_socket)
            .await
        {}
    }

    pub async fn unified_listener_task(
        &mut self,
        java_listener: &mut TcpListener,
        bedrock_listener: &mut UdpSocket,
        server_hand: Sender<NetResponse>,
    ) -> bool {
        let mut udp_buf = [0; 1496]; // Buffer for UDP receive
        let bedrock_clients = DashMap::new();

        select! {
            // Branch for TCP connections (Java Edition)
            tcp_result = java_listener.accept() => {
                self.process_java_packet(tcp_result).await;
            },

            // Branch for UDP packets (Bedrock Edition)
            udp_result = bedrock_listener.recv_from(&mut udp_buf) => {
                self.process_bedrock_packet(udp_result, &mut udp_buf).await;
            },

            // Branch for the global stop signal
            () = STOP_INTERRUPT.notified() => {
                return false;
            }
        }
        true
    }

    async fn process_java_packet(&mut self, tcp_result: Result<(TcpStream, SocketAddr), Error>) {
        match tcp_result {
            Ok((connection, client_addr)) => {
                if let Err(e) = connection.set_nodelay(true) {
                    log::warn!("Failed to set TCP_NODELAY: {e}");
                }

                let client_id = self.master_client_id_counter;
                self.master_client_id_counter += 1;

                let formatted_address = if BASIC_CONFIG.scrub_ips {
                    scrub_address(&format!("{client_addr}"))
                } else {
                    format!("{client_addr}")
                };
                log::debug!(
                    "Accepted connection from Java Edition: {formatted_address} (id {client_id})"
                );

                let (client_handle, server_handle) = make_client_channel();
                self.server_hand.send(NetResponse::AddPlayer(server_handle));
                let key_store = self.key_store.clone();

                self.tracker.spawn(async move {
                    let mut java_client = JavaClient::new(
                        connection,
                        client_addr,
                        client_id,
                        client_handle,
                        key_store,
                    );
                    java_client.run().await;
                });
            }
            Err(e) => {
                log::error!("Failed to accept Java client connection: {e}");
            }
        }
    }

    async fn process_bedrock_packet(
        &mut self,
        udp_result: Result<(usize, SocketAddr), Error>,
        udp_buf: &mut [u8],
        master_client_id_counter: &mut u64,
    ) {
        match udp_result {
            Ok((len, client_addr)) => {
                if len == 0 {
                    log::warn!("Received empty UDP packet from {client_addr}");
                } else {
                    let id = udp_buf[0];
                    let is_online = id & 128 != 0;

                    if is_online {
                        if let Some(client) = self.bedrock_clients.get_mut(&client_addr) {
                            let reader = Cursor::new(udp_buf[..len].to_vec());

                            client.process_packet(reader).await;
                        } else if let Ok(packet) =
                            BedrockClient::is_connection_request(&mut Cursor::new(&udp_buf[4..len]))
                        {
                            *master_client_id_counter += 1;

                            let (client_handle, server_handle) = make_client_channel();
                            let mut platform = BedrockClient::new(server_handle, client_addr);
                            platform.handle_connection_request(packet).await;
                            platform.start_outgoing_packet_task();

                            self.bedrock_clients.insert(client_addr, client_handle);
                        }
                    } else {
                        // Please keep the function as simple as possible!
                        // We dont care about the result, the client just resends the packet
                        // Since offline packets are very small we dont need to move and clone the data
                        let _ = BedrockClient::handle_offline_packet(
                            &self.server,
                            id,
                            &mut Cursor::new(&udp_buf[1..len]),
                            client_addr,
                            &self.udp_socket,
                        )
                        .await;
                    }
                }
            }
            // Since all packets go over this match statement, there should be not waiting
            Err(e) => {
                log::error!("{e}");
            }
        }
    }
}

fn make_client_channel() -> (ClientHandle, ServerHandle) {
    let (send1, recv1) = unbounded_channel();
    let (send2, recv2) = unbounded_channel();
    (
        ClientHandle {
            sender: send1,
            receiver: recv2,
        },
        ServerHandle {
            sender: send2,
            receiver: recv1,
        },
    )
}

impl ServerHandle {
    pub fn send(&mut self, msg: ServerClientEvent) -> Result<(), SendError<ServerClientEvent>> {
        self.sender.send(msg)
    }
}

impl ClientHandle {
    pub fn send(&mut self, msg: ClientServerEvent) -> Result<(), SendError<ClientServerEvent>> {
        self.sender.send(msg)
    }
}

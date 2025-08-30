use std::error::Error;
use std::fmt::Display;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{io::Write, sync::Arc};

use bytes::Bytes;
use pumpkin_config::networking::compression::CompressionInfo;
use pumpkin_protocol::PacketEncodeError;
use pumpkin_protocol::java::server::play::{
    SChangeGameMode, SChatCommand, SChatMessage, SChunkBatch, SClickSlot, SClientCommand,
    SClientInformationPlay, SClientTickEnd, SCloseContainer, SCommandSuggestion, SConfirmTeleport,
    SCookieResponse as SPCookieResponse, SCustomPayload, SInteract, SKeepAlive, SPickItemFromBlock,
    SPlayPingRequest, SPlayerAbilities, SPlayerAction, SPlayerCommand, SPlayerInput, SPlayerLoaded,
    SPlayerPosition, SPlayerPositionRotation, SPlayerRotation, SPlayerSession, SSetCommandBlock,
    SSetCreativeSlot, SSetHeldItem, SSetPlayerGround, SSwingArm, SUpdateSign, SUseItem, SUseItemOn,
};
use pumpkin_protocol::{
    ClientPacket, ConnectionState, PacketDecodeError, RawPacket, ServerPacket,
    codec::var_int::VarInt,
    java::{
        client::{config::CConfigDisconnect, login::CLoginDisconnect, play::CPlayDisconnect},
        packet_decoder::TCPNetworkDecoder,
        packet_encoder::TCPNetworkEncoder,
        server::{
            config::{
                SAcknowledgeFinishConfig, SClientInformationConfig, SConfigCookieResponse,
                SConfigResourcePack, SKnownPacks, SPluginMessage,
            },
            handshake::SHandShake,
            login::{
                SEncryptionResponse, SLoginAcknowledged, SLoginCookieResponse,
                SLoginPluginResponse, SLoginStart,
            },
            status::{SStatusPingRequest, SStatusRequest},
        },
    },
    packet::Packet,
    ser::{NetworkWriteExt, ReadingError, WritingError},
};
use pumpkin_util::text::TextComponent;
use tokio::sync::Notify;
use tokio::{
    io::{BufReader, BufWriter},
    net::{
        TcpStream,
        tcp::{OwnedReadHalf, OwnedWriteHalf},
    },
    sync::Mutex,
};
use tokio::{
    sync::mpsc::{Receiver, Sender},
    task::JoinHandle,
};
use tokio_util::task::TaskTracker;

pub mod config;
pub mod handshake;
pub mod login;
pub mod play;
pub mod status;

use crate::entity::player::Player;
use crate::net::key_store::KeyStore;
use crate::net::net_thread::{ClientHandle, ClientServerEvent};
use crate::net::proxy::bungeecord::BungeeCordError;
use crate::net::proxy::velocity::VelocityError;
use crate::net::{GameProfile, PlayerConfig};
use crate::{error::PumpkinError, net::EncryptionError, server::Server};

type NetEncoder = TCPNetworkEncoder<BufWriter<OwnedWriteHalf>>;
type NetDecoder = TCPNetworkDecoder<BufReader<OwnedReadHalf>>;

pub struct LoginClient {
    pub id: u64,
    /// The packet encoder for outgoing packets.
    pub network_writer: NetEncoder,
    /// The packet decoder for incoming packets.
    pub network_reader: NetDecoder,
    pub address: SocketAddr,
    pub server_address: String,
    pub key_store: Arc<KeyStore>,
}

pub struct JavaClient {
    pub id: u64,
    /// The client's game profile information.
    pub gameprofile: Mutex<GameProfile>,
    /// The client's configuration settings, Optional
    pub config: Mutex<Option<PlayerConfig>>,
    /// The Address used to connect to the Server, Send in the Handshake
    pub server_address: Mutex<String>,
    server_conn: ClientHandle,
    /// The current connection state of the client (e.g., Handshaking, Status, Play).
    pub connection_state: ConnectionState,
    /// Indicates if the client connection is closed.
    pub closed: Arc<AtomicBool>,
    /// The client's IP address.
    pub address: Mutex<SocketAddr>,
    /// The client's brand or modpack information, Optional.
    pub brand: Mutex<String>,
    /// A collection of tasks associated with this client. The tasks await completion when removing the client.
    tasks: TaskTracker,
    /// An notifier that is triggered when this client is closed.
    close_interrupt: Arc<Notify>,
    /// A queue of serialized packets to send to the network
    outgoing_packet_queue_send: Sender<Bytes>,
    /// A queue of serialized packets to send to the network
    outgoing_packet_queue_recv: Option<Receiver<Bytes>>,
    /// The packet encoder for outgoing packets.
    network_writer: NetEncoder,
    /// The packet decoder for incoming packets.
    network_reader: NetDecoder,
    key_store: Arc<KeyStore>,
}

impl JavaClient {
    #[must_use]
    pub async fn new(
        tcp_stream: TcpStream,
        address: SocketAddr,
        id: u64,
        server_conn: ClientHandle,
        key_store: Arc<KeyStore>,
    ) -> Self {
        let (read, write) = tcp_stream.into_split();
        let (send, recv) = tokio::sync::mpsc::channel(128);

        let mut network_writer = TCPNetworkEncoder::new(BufWriter::new(write));
        let mut network_reader = TCPNetworkDecoder::new(BufReader::new(read));

        let login_client = LoginClient {
            id,
            network_writer,
            network_reader,
            address,
            server_address: "".into(),
            key_store,
        };

        let login_details = login_client.process_login().await;

        Self {
            id,
            gameprofile: Mutex::new(None),
            config: Mutex::new(None),
            server_address: Mutex::new(String::new()),
            server_conn,
            address: Mutex::new(address),
            connection_state: ConnectionState::HandShake,
            closed: Arc::new(AtomicBool::new(false)),
            close_interrupt: Arc::new(Notify::new()),
            tasks: TaskTracker::new(),
            outgoing_packet_queue_send: send,
            outgoing_packet_queue_recv: Some(recv),

            network_writer,
            network_reader,
            brand: Mutex::new(None),
            key_store,
        }
    }

    pub async fn run(&mut self) {
        self.start_outgoing_packet_task().await;

        self.close();
        self.await_tasks().await;

        let player = self.player.lock();
        if let Some(player) = player.as_ref() {
            log::debug!("Cleaning up player for id {client_id}");

            if let Err(e) = server_clone
                .player_data_storage
                .handle_player_leave(player)
                .await
            {
                log::error!("Failed to save player data on disconnect: {e}");
            }

            player.remove();
            server_clone.remove_player(player).await;
        } else if java_client.connection_state == Play {
            log::error!("No player found for id {client_id}. This should not happen!");
        }
    }

    pub async fn set_compression(&mut self, compression: CompressionInfo) {
        if compression.level > 9 {
            log::error!("Invalid compression level! Clients will not be able to read this!");
        }

        self.network_reader
            .set_compression(compression.threshold as usize);

        self.network_writer
            .set_compression((compression.threshold as usize, compression.level));
    }

    pub async fn await_tasks(&self) {
        self.tasks.close();
        self.tasks.wait().await;
    }

    /// Spawns a task associated with this client. All tasks spawned with this method are awaited
    /// when the client. This means tasks should complete in a reasonable amount of time or select
    /// on `Self::await_close_interrupt` to cancel the task when the client is closed
    ///
    /// Returns an `Option<JoinHandle<F::Output>>`. If the client is closed, this returns `None`.
    pub fn spawn_task<F>(&self, task: F) -> Option<JoinHandle<F::Output>>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        if self.closed.load(Ordering::Relaxed) {
            None
        } else {
            Some(self.tasks.spawn(task))
        }
    }

    pub async fn enqueue_packet<P: ClientPacket>(&self, packet: &P) {
        let mut buf = Vec::new();
        let writer = &mut buf;
        Self::write_packet(packet, writer).unwrap();
        self.enqueue_packet_data(buf.into()).await;
    }

    /// Queues a clientbound packet to be sent to the connected client. Queued chunks are sent
    /// in-order to the client
    ///
    /// # Arguments
    ///
    /// * `packet`: A reference to a packet object implementing the `ClientPacket` trait.
    pub async fn enqueue_packet_data(&self, packet_data: Bytes) {
        if let Err(err) = self.outgoing_packet_queue_send.send(packet_data).await {
            // This is expected to fail if we are closed
            if !self.closed.load(Ordering::Relaxed) {
                log::error!(
                    "Failed to add packet to the outgoing packet queue for client {}: {}",
                    self.id,
                    err
                );
            }
        }
    }

    pub async fn await_close_interrupt(&self) {
        self.close_interrupt.notified().await;
    }

    pub async fn kick(&self, reason: TextComponent) {
        match self.connection_state {
            ConnectionState::Login => {
                // TextComponent implements Serialize and writes in bytes instead of String, that's the reasib we only use content
                self.send_packet_now(&CLoginDisconnect::new(
                    serde_json::to_string(&reason.0).unwrap_or_else(|_| String::new()),
                ))
                .await;
            }
            ConnectionState::Config => {
                self.send_packet_now(&CConfigDisconnect::new(&reason.get_text()))
                    .await;
            }
            ConnectionState::Play => self.send_packet_now(&CPlayDisconnect::new(&reason)).await,
            _ => {}
        }
        log::debug!("Closing connection for {}", self.id);
        self.close();
    }

    /// Handles an incoming packet, routing it to the appropriate handler based on the current connection state.
    ///
    /// This function takes a `RawPacket` and routes it to the corresponding handler based on the current connection state.
    /// It supports the following connection states:
    ///
    /// - **Handshake:** Handles handshake packets.
    /// - **Status:** Handles status request and ping packets.
    /// - **Login/Transfer:** Handles login and transfer packets.
    /// - **Config:** Handles configuration packets.
    ///
    /// For the `Play` state, an error is logged as it indicates an invalid state for packet processing.
    ///
    /// # Arguments
    ///
    /// * `server`: A reference to the `Server` instance.
    /// * `packet`: A mutable reference to the `RawPacket` to be processed.
    ///
    /// # Returns
    ///
    /// A `Result` indicating whether the packet was read and handled successfully.
    ///
    /// # Errors
    ///
    /// Returns a `DeserializerError` if an error occurs during packet deserialization.
    pub async fn handle_packet(&mut self, packet: &RawPacket) -> Result<(), ReadingError> {
        match self.connection_state {
            ConnectionState::HandShake => self.handle_handshake_packet(packet).await,
            ConnectionState::Status => self.handle_status_packet(packet).await,
            // TODO: Check config if transfer is enabled
            ConnectionState::Login | ConnectionState::Transfer => {
                self.handle_login_packet(packet).await
            }
            ConnectionState::Config => self.handle_config_packet(packet).await,
            ConnectionState::Play => {
                if let Some(player) = self.player.lock().await.as_ref() {
                    match self.handle_play_packet(player, packet).await {
                        Ok(()) => {}
                        Err(e) => {
                            if e.is_kick() {
                                if let Some(kick_reason) = e.client_kick_reason() {
                                    self.kick(TextComponent::text(kick_reason)).await;
                                } else {
                                    self.kick(TextComponent::text(format!(
                                        "Error while handling incoming packet {e}"
                                    )))
                                    .await;
                                }
                            }
                            e.log();
                        }
                    }
                }
                Ok(())
            }
        }
    }

    async fn handle_handshake_packet(&self, packet: &RawPacket) -> Result<(), ReadingError> {
        log::debug!("Handling handshake group");
        let payload = &packet.payload[..];
        match packet.id {
            0 => {
                self.handle_handshake(SHandShake::read(payload)?).await;
                Ok(())
            }
            _ => Err(ReadingError::Message(format!(
                "Failed to handle packet id {} in Handshake State",
                packet.id
            ))),
        }
    }

    async fn handle_status_packet(&mut self, packet: &RawPacket) -> Result<(), ReadingError> {
        log::debug!("Handling status group");
        let payload = &packet.payload[..];
        match packet.id {
            SStatusRequest::PACKET_ID => {
                self.server_conn.send(ClientServerEvent::StatusRequest);
                Ok(())
            }
            SStatusPingRequest::PACKET_ID => {
                self.handle_ping_request(SStatusPingRequest::read(payload)?)
                    .await;
                Ok(())
            }
            _ => Err(ReadingError::Message(format!(
                "Failed to handle java client packet id {} in Status State",
                packet.id
            ))),
        }
    }

    /// Processes all packets received from the connected client in a loop.
    ///
    /// This function continuously dequeues packets from the client's packet queue and processes them.
    /// Processing involves calling the `handle_packet` function with the server instance and the packet itself.
    ///
    /// The loop exits when:
    ///
    /// - The connection is closed (checked before processing each packet).
    /// - An error occurs while processing a packet (client is kicked with an error message).
    pub async fn start_outgoing_packet_task(&mut self) {
        let mut packet_receiver = self
            .outgoing_packet_queue_recv
            .take()
            .expect("This was set in the new fn");
        let close_interrupt = self.close_interrupt.clone();
        let closed = self.closed.clone();
        let mut keep_running = true;

        while !closed.load(Ordering::Relaxed) && keep_running {
            keep_running = tokio::select! {
                () = close_interrupt.notified() => {
                    false
                },
                recv_result = packet_receiver.recv() => {
                    self.process_send(recv_result).await
                }
                packet_result = self.network_reader.get_raw_packet() => {
                    self.read_packet(packet_result).await
                }
            };
        }
    }

    /// return should continue
    pub async fn process_send(&mut self, recv_result: Option<Bytes>) -> bool {
        let writer = &mut self.network_writer;
        let Some(recv_result) = recv_result else {
            return true;
        };
        if let Err(err) = writer.write_packet(recv_result).await {
            // It is expected that the packet will fail if we are closed
            if !self.closed.load(Ordering::Relaxed) {
                log::warn!("Failed to send packet to client {}: {err}", self.id);
                // We now need to close the connection to the client since the stream is in an
                // unknown state
                self.close_interrupt.notify_waiters();
                self.closed.store(true, Ordering::Relaxed);
                return false;
            }
        }
        true
    }

    pub async fn read_packet(
        &mut self,
        packet_result: Result<RawPacket, PacketDecodeError>,
    ) -> bool {
        match packet_result {
            Ok(packet) => {
                if let Err(error) = self.handle_packet(&packet).await {
                    let text = format!("Error while reading incoming packet {error}");
                    log::error!(
                        "Failed to read incoming packet with id {}: {}",
                        packet.id,
                        error
                    );
                    self.kick(TextComponent::text(text)).await;
                    return false;
                }
            }
            Err(err) => {
                if !matches!(err, PacketDecodeError::ConnectionClosed) {
                    log::warn!("Failed to decode packet from client {}: {}", self.id, err);
                    let text = format!("Error while reading incoming packet {err}");
                    self.kick(TextComponent::text(text)).await;
                }
                return false;
            }
        }
        true
    }

    /// Closes the connection to the client.
    ///
    /// This function marks the connection as closed using an atomic flag. It's generally preferable
    /// to use the `kick` function if you want to send a specific message to the client explaining the reason for the closure.
    /// However, use `close` in scenarios where sending a message is not critical or might not be possible (e.g., sudden connection drop).
    ///
    /// # Notes
    ///
    /// This function does not attempt to send any disconnect packets to the client.
    pub fn close(&self) {
        self.close_interrupt.notify_waiters();
        self.closed.store(true, Ordering::Relaxed);
    }

    async fn handle_login_handshake(&self, packet: &RawPacket) -> Result<(), ReadingError> {
        log::debug!("Handling login group for id");
        let payload = &packet.payload[..];
        match packet.id {
            SLoginStart::PACKET_ID => {
                self.handle_login_start(SLoginStart::read(payload)?).await;
            }
            SEncryptionResponse::PACKET_ID => {
                self.handle_encryption_response(SEncryptionResponse::read(payload)?)
                    .await;
            }
            SLoginPluginResponse::PACKET_ID => {
                self.handle_plugin_response(SLoginPluginResponse::read(payload)?)
                    .await;
            }
            SLoginAcknowledged::PACKET_ID => {
                self.handle_login_acknowledged().await;
            }
            SLoginCookieResponse::PACKET_ID => {
                self.handle_login_cookie_response(&SLoginCookieResponse::read(payload)?);
            }
            _ => {
                log::error!(
                    "Failed to handle java client packet id {} in Login State",
                    packet.id
                );
            }
        }
        Ok(())
    }

    async fn handle_config_packet(&mut self, packet: &RawPacket) -> Result<(), ReadingError> {
        log::debug!("Handling config group");
        let payload = &packet.payload[..];
        match packet.id {
            SClientInformationConfig::PACKET_ID => {
                self.handle_client_information_config(SClientInformationConfig::read(payload)?)
                    .await;
            }
            SPluginMessage::PACKET_ID => {
                self.handle_plugin_message(SPluginMessage::read(payload)?)
                    .await;
            }
            SAcknowledgeFinishConfig::PACKET_ID => {
                self.handle_config_acknowledged().await;
            }
            SKnownPacks::PACKET_ID => {
                self.handle_known_packs(SKnownPacks::read(payload)?).await;
            }
            SConfigCookieResponse::PACKET_ID => {
                self.handle_config_cookie_response(&SConfigCookieResponse::read(payload)?);
            }
            SConfigResourcePack::PACKET_ID => {
                self.handle_resource_pack_response(SConfigResourcePack::read(payload)?)
                    .await;
            }
            _ => {
                log::error!(
                    "Failed to handle java client packet id {} in Config State",
                    packet.id
                );
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    pub async fn handle_play_packet(
        &self,
        player: &Arc<Player>,
        server: &Arc<Server>,
        packet: &RawPacket,
    ) -> Result<(), Box<dyn PumpkinError>> {
        let payload = &packet.payload[..];
        match packet.id {
            SConfirmTeleport::PACKET_ID => {
                self.handle_confirm_teleport(player, SConfirmTeleport::read(payload)?)
                    .await;
            }
            SChangeGameMode::PACKET_ID => {
                self.handle_change_game_mode(player, SChangeGameMode::read(payload)?)
                    .await;
            }
            SChatCommand::PACKET_ID => {
                self.handle_chat_command(player, server, &(SChatCommand::read(payload)?))
                    .await;
            }
            SChatMessage::PACKET_ID => {
                self.handle_chat_message(player, SChatMessage::read(payload)?)
                    .await;
            }
            SClientInformationPlay::PACKET_ID => {
                self.handle_client_information(player, SClientInformationPlay::read(payload)?)
                    .await;
            }
            SClientCommand::PACKET_ID => {
                self.handle_client_status(player, SClientCommand::read(payload)?)
                    .await;
            }
            SPlayerInput::PACKET_ID => {
                self.handle_player_input(player, SPlayerInput::read(payload)?)
                    .await;
            }
            SInteract::PACKET_ID => {
                self.handle_interact(player, SInteract::read(payload)?, server)
                    .await;
            }
            SKeepAlive::PACKET_ID => {
                self.handle_keep_alive(player, SKeepAlive::read(payload)?)
                    .await;
            }
            SClientTickEnd::PACKET_ID => {
                // TODO
            }
            SPlayerPosition::PACKET_ID => {
                self.handle_position(player, SPlayerPosition::read(payload)?)
                    .await;
            }
            SPlayerPositionRotation::PACKET_ID => {
                self.handle_position_rotation(player, SPlayerPositionRotation::read(payload)?)
                    .await;
            }
            SPlayerRotation::PACKET_ID => {
                self.handle_rotation(player, SPlayerRotation::read(payload)?)
                    .await;
            }
            SSetPlayerGround::PACKET_ID => {
                self.handle_player_ground(player, &SSetPlayerGround::read(payload)?);
            }
            SPickItemFromBlock::PACKET_ID => {
                self.handle_pick_item_from_block(player, SPickItemFromBlock::read(payload)?)
                    .await;
            }
            SPlayerAbilities::PACKET_ID => {
                self.handle_player_abilities(player, SPlayerAbilities::read(payload)?)
                    .await;
            }
            SPlayerAction::PACKET_ID => {
                self.handle_player_action(player, SPlayerAction::read(payload)?, server)
                    .await;
            }
            SSetCommandBlock::PACKET_ID => {
                self.handle_set_command_block(player, SSetCommandBlock::read(payload)?)
                    .await;
            }
            SPlayerCommand::PACKET_ID => {
                self.handle_player_command(player, SPlayerCommand::read(payload)?)
                    .await;
            }
            SPlayerLoaded::PACKET_ID => Self::handle_player_loaded(player),
            SPlayPingRequest::PACKET_ID => {
                self.handle_play_ping_request(SPlayPingRequest::read(payload)?)
                    .await;
            }
            SClickSlot::PACKET_ID => {
                player.on_slot_click(SClickSlot::read(payload)?).await;
            }
            SSetHeldItem::PACKET_ID => {
                self.handle_set_held_item(player, SSetHeldItem::read(payload)?)
                    .await;
            }
            SSetCreativeSlot::PACKET_ID => {
                self.handle_set_creative_slot(player, SSetCreativeSlot::read(payload)?)
                    .await?;
            }
            SSwingArm::PACKET_ID => {
                self.handle_swing_arm(player, SSwingArm::read(payload)?)
                    .await;
            }
            SUpdateSign::PACKET_ID => {
                self.handle_sign_update(player, SUpdateSign::read(payload)?)
                    .await;
            }
            SUseItemOn::PACKET_ID => {
                self.handle_use_item_on(player, SUseItemOn::read(payload)?, server)
                    .await?;
            }
            SUseItem::PACKET_ID => {
                self.handle_use_item(player, &SUseItem::read(payload)?, server)
                    .await;
            }
            SCommandSuggestion::PACKET_ID => {
                self.handle_command_suggestion(player, SCommandSuggestion::read(payload)?, server)
                    .await;
            }
            SPCookieResponse::PACKET_ID => {
                self.handle_cookie_response(&SPCookieResponse::read(payload)?);
            }
            SCloseContainer::PACKET_ID => {
                self.handle_close_container(player, server, SCloseContainer::read(payload)?)
                    .await;
            }
            SChunkBatch::PACKET_ID => {
                self.handle_chunk_batch(player, SChunkBatch::read(payload)?)
                    .await;
            }
            SPlayerSession::PACKET_ID => {
                self.handle_chat_session_update(player, server, SPlayerSession::read(payload)?)
                    .await;
            }
            SCustomPayload::PACKET_ID => {
                // TODO: this fixes Failed to handle player packet id for now
            }
            _ => {
                log::warn!("Failed to handle player packet id {}", packet.id);
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
enum LoginError {
    InvalidHandshake,
    ServerRejected,
    InvalidUsername,
    IgnoredPluginRequest,
    ReadError(ReadingError),
    VelocityError(VelocityError),
    BungeeCordError(BungeeCordError),
}

impl LoginClient {
    /// Start the login handshake
    ///
    /// Handle this sequentially (async w.r.t server) to reduce need for stored state and synchronisation
    async fn process_login(&mut self) -> Result<(), LoginError> {
        self.network_reader.get_raw_packet();
        let Ok(packet) = self.network_reader.get_raw_packet().await else {
            return Err(LoginError::InvalidHandshake);
        };
        if packet.id != SLoginStart::PACKET_ID {
            return Err(LoginError::InvalidHandshake);
        }
        self.handle_login_start(SLoginStart::read(&packet.payload[..])?)
            .await?;
        let Ok(packet) = self.network_reader.get_raw_packet().await else {
            return Err(LoginError::InvalidHandshake);
        };
        Ok(())
    }

    pub async fn set_encryption(
        &mut self,
        shared_secret: &[u8], // decrypted
    ) -> Result<(), EncryptionError> {
        let crypt_key: [u8; 16] = shared_secret
            .try_into()
            .map_err(|_| EncryptionError::SharedWrongLength)?;
        self.network_reader.set_encryption(&crypt_key);
        self.network_writer.set_encryption(&crypt_key);
        Ok(())
    }

    pub async fn kick(&mut self, reason: TextComponent) -> Result<(), PacketEncodeError> {
        send_packet_now(
            &mut self.network_writer,
            &CLoginDisconnect::new(
                serde_json::to_string(&reason.0).unwrap_or_else(|_| String::new()),
            ),
        )
        .await
    }
}

pub async fn send_packet_now<P: ClientPacket>(
    network_writer: &mut NetEncoder,
    packet: &P,
) -> Result<(), PacketEncodeError> {
    let mut packet_buf = Vec::new();
    let writer = &mut packet_buf;
    write_packet(packet, writer).unwrap();
    send_packet_now_data(network_writer, packet_buf).await
}

pub async fn send_packet_now_data(
    network_writer: &mut NetEncoder,
    packet: Vec<u8>,
) -> Result<(), PacketEncodeError> {
    network_writer.write_packet(packet.into()).await
}

pub fn write_packet<P: ClientPacket>(packet: &P, write: impl Write) -> Result<(), WritingError> {
    let mut write = write;
    write.write_var_int(&VarInt(P::PACKET_ID))?;
    packet.write_packet_data(write)
}

impl LoginError {
    fn get_kick_message(&self) -> TextComponent {
        match self {
            LoginError::InvalidHandshake => {
                TextComponent::text("Invalid packet performing Handshake!")
            }
            LoginError::ServerRejected => {
                TextComponent::text("Rejected by server, server full or UUID/username duplicate")
            }
            LoginError::InvalidUsername => TextComponent::text("Invalid characters in username"),
            LoginError::ReadError(reading_error) => TextComponent::text("Internal server error"),
        }
    }
}

impl From<ReadingError> for LoginError {
    fn from(value: ReadingError) -> Self {
        Self::ReadError(value)
    }
}

impl From<VelocityError> for LoginError {
    fn from(value: VelocityError) -> Self {
        Self::VelocityError(value)
    }
}

impl From<BungeeCordError> for LoginError {
    fn from(value: BungeeCordError) -> Self {
        Self::BungeeCordError(value)
    }
}

impl Error for LoginError {}
impl Display for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoginError::ReadError(reading_error) => write!(f, "ReadError {{ {reading_error} }}"),
            LoginError::InvalidHandshake => write!(f, "InvalidHandshake"),
            LoginError::ServerRejected => write!(f, "ServerRejected"),
            LoginError::InvalidUsername => write!(f, "InvalidUsername"),
            LoginError::VelocityError(vel) => write!(f, "VelocityError {{ {vel} }}"),
            LoginError::BungeeCordError(bung) => write!(f, "BungeeCordError {{ {bung} }}"),
            LoginError::IgnoredPluginRequest => write!(f, "IgnoredPluginRequest"),
        }
    }
}

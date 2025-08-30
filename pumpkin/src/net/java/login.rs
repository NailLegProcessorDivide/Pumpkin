use std::{net::SocketAddr, sync::LazyLock};

use pumpkin_config::{BASIC_CONFIG, advanced_config, networking::proxy::ProxyType};
use pumpkin_protocol::{
    ConnectionState, KnownPack, Label, Link, LinkType,
    java::client::{
        config::{CConfigAddResourcePack, CConfigServerLinks, CKnownPacks, CUpdateTags},
        login::{CLoginSuccess, CSetCompression},
    },
    java::server::login::{
        SEncryptionResponse, SLoginCookieResponse, SLoginPluginResponse, SLoginStart,
    },
};
use pumpkin_util::text::TextComponent;
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::{
    net::{
        GameProfile,
        authentication::{self, AuthError},
        is_valid_player_name,
        java::{self, LoginClient, LoginError},
        net_thread::{ClientHandle, ClientServerEvent},
        offline_uuid,
        proxy::{bungeecord, velocity},
    },
    server::Server,
};

static LINKS: LazyLock<Vec<Link>> = LazyLock::new(|| {
    let mut links: Vec<Link> = Vec::new();

    let bug_report = &advanced_config().server_links.bug_report;
    if !bug_report.is_empty() {
        links.push(Link::new(Label::BuiltIn(LinkType::BugReport), bug_report));
    }

    let support = &advanced_config().server_links.support;
    if !support.is_empty() {
        links.push(Link::new(Label::BuiltIn(LinkType::Support), support));
    }

    let status = &advanced_config().server_links.status;
    if !status.is_empty() {
        links.push(Link::new(Label::BuiltIn(LinkType::Status), status));
    }

    let feedback = &advanced_config().server_links.feedback;
    if !feedback.is_empty() {
        links.push(Link::new(Label::BuiltIn(LinkType::Feedback), feedback));
    }

    let community = &advanced_config().server_links.community;
    if !community.is_empty() {
        links.push(Link::new(Label::BuiltIn(LinkType::Community), community));
    }

    let website = &advanced_config().server_links.website;
    if !website.is_empty() {
        links.push(Link::new(Label::BuiltIn(LinkType::Website), website));
    }

    let forums = &advanced_config().server_links.forums;
    if !forums.is_empty() {
        links.push(Link::new(Label::BuiltIn(LinkType::Forums), forums));
    }

    let news = &advanced_config().server_links.news;
    if !news.is_empty() {
        links.push(Link::new(Label::BuiltIn(LinkType::News), news));
    }

    let announcements = &advanced_config().server_links.announcements;
    if !announcements.is_empty() {
        links.push(Link::new(
            Label::BuiltIn(LinkType::Announcements),
            announcements,
        ));
    }

    for (key, value) in &advanced_config().server_links.custom {
        links.push(Link::new(
            Label::TextComponent(TextComponent::text(key).into()),
            value,
        ));
    }
    links
});

impl LoginClient {
    pub async fn handle_login_start(
        &mut self,
        login_start: SLoginStart,
    ) -> Result<(GameProfile, SocketAddr), LoginError> {
        log::debug!("login start");

        // Don't allow new logons when the server is full.
        // If `max_players` is set to zero, then there is no max player count enforced.
        // TODO: If client is an operator or has otherwise suitable elevated permissions, allow the client to bypass this requirement.
        let max_players = BASIC_CONFIG.max_players;
        // TODO: Actually get info from server thread
        if !max_players.is_some_and(|num| num.get() < 5) {
            self.kick(TextComponent::translate(
                "multiplayer.disconnect.server_full",
                [],
            ))
            .await;
            return Err(LoginError::ServerRejected);
        }

        if !is_valid_player_name(&login_start.name) {
            return Err(LoginError::InvalidUsername);
        }
        // Default game profile, when no online mode
        // TODO: Make offline UUID
        match &advanced_config().networking.proxy {
            Some(ProxyType::Velocity { secret: _ }) => {
                velocity::velocity_login(self).await;
                todo!("make a game profile when connecting to a velocity proxy")
            }
            Some(ProxyType::BengeeCord) => Ok(bungeecord::bungeecord_login(
                &self.address,
                &self.server_address,
                login_start.name,
            )
            .await
            .map(|(_, profile)| (profile, self.address))?),
            None => self.process_vanilla_login_start(login_start).await,
        }
    }

    async fn process_vanilla_login_start(
        &mut self,
        login_start: SLoginStart,
    ) -> Result<(GameProfile, SocketAddr), LoginError> {
        let id = if BASIC_CONFIG.online_mode {
            login_start.uuid
        } else {
            offline_uuid(&login_start.name).expect("This is very not safe and bad")
        };

        let profile = GameProfile {
            id,
            name: login_start.name,
            properties: vec![],
            profile_actions: None,
        };

        if advanced_config().networking.packet_compression.enabled {
            self.enable_compression().await;
        }

        if BASIC_CONFIG.encryption {
            let verify_token: [u8; 4] = rand::random();
            // Wait until we have sent the encryption packet to the client
            java::send_packet_now(
                &mut self.network_writer,
                &self
                    .key_store
                    .encryption_request("", &verify_token, BASIC_CONFIG.online_mode),
            )
            .await;
        } else {
            self.finish_login(&profile).await;
        }
        Ok((profile, self.address))
    }

    pub async fn handle_encryption_response(&mut self, encryption_response: SEncryptionResponse) {
        log::debug!("Handling encryption");
        let shared_secret = self
            .key_store
            .decrypt(&encryption_response.shared_secret)
            .unwrap();

        if let Err(error) = self.set_encryption(&shared_secret).await {
            self.kick(TextComponent::text(error.to_string())).await;
            return;
        }

        let mut gameprofile = self.gameprofile.lock().await;

        let Some(profile) = gameprofile.as_mut() else {
            self.kick(TextComponent::text("No `GameProfile`")).await;
            return;
        };

        if BASIC_CONFIG.online_mode {
            // Online mode auth
            match self.authenticate(&shared_secret, &profile.name).await {
                Ok(new_profile) => *profile = new_profile,
                Err(error) => {
                    self.kick(match error {
                        AuthError::FailedResponse => {
                            TextComponent::translate("multiplayer.disconnect.authservers_down", [])
                        }
                        AuthError::UnverifiedUsername => TextComponent::translate(
                            "multiplayer.disconnect.unverified_username",
                            [],
                        ),
                        e => TextComponent::text(e.to_string()),
                    })
                    .await;
                }
            }
        }

        if Self::check_player_exists(&mut self.server_conn, profile.name.clone(), profile.id).await
        {
            log::debug!(
                "Player (IP '{}', username '{}') tried to log in with the same UUID or username as an online player (uuid '{}')",
                &self.address,
                &profile.name,
                &profile.id,
            );
            self.kick(TextComponent::translate(
                "multiplayer.disconnect.duplicate_login",
                [],
            ))
            .await;
            return;
        }

        self.finish_login(profile).await;
    }

    async fn check_player_exists(server_conn: &mut ClientHandle, name: String, uuid: Uuid) -> bool {
        let (sender, receiver) = oneshot::channel();
        server_conn
            .send(ClientServerEvent::CanPlayerJoin(name, uuid, sender))
            .is_err()
            || receiver.await != Ok(true)
    }

    async fn enable_compression(&mut self) {
        let compression = advanced_config().networking.packet_compression.info.clone();
        // We want to wait until we have sent the compression packet to the client
        self.send_packet_now(&CSetCompression::new(
            compression.threshold.try_into().unwrap(),
        ))
        .await;
        self.set_compression(compression).await;
    }

    async fn finish_login(&self, profile: &GameProfile) {
        let packet = CLoginSuccess::new(&profile.id, &profile.name, &profile.properties);
        self.send_packet_now(&packet).await;
    }

    async fn authenticate(
        &self,
        shared_secret: &[u8],
        username: &str,
    ) -> Result<GameProfile, AuthError> {
        let hash = self.key_store.get_digest(shared_secret);
        let ip = self.address.lock().await.ip();
        let profile = authentication::authenticate(username, &hash, &ip)?;

        // Check if the player should join
        if let Some(actions) = &profile.profile_actions {
            if advanced_config()
                .networking
                .authentication
                .player_profile
                .allow_banned_players
            {
                for allowed in &advanced_config()
                    .networking
                    .authentication
                    .player_profile
                    .allowed_actions
                {
                    if !actions.contains(allowed) {
                        return Err(AuthError::DisallowedAction);
                    }
                }
                if !actions.is_empty() {
                    return Err(AuthError::Banned);
                }
            } else if !actions.is_empty() {
                return Err(AuthError::Banned);
            }
        }
        // Validate textures
        for property in &profile.properties {
            authentication::validate_textures(
                property,
                &advanced_config().networking.authentication.textures,
            )
            .map_err(AuthError::TextureError)?;
        }
        Ok(profile)
    }

    pub fn handle_login_cookie_response(&self, packet: &SLoginCookieResponse) {
        // TODO: allow plugins to access this
        log::debug!(
            "Received cookie_response[login]: key: \"{}\", payload_length: \"{:?}\"",
            packet.key,
            packet.payload.as_ref().map(|p| p.len())
        );
    }
    pub async fn handle_plugin_response(
        &self,
        plugin_response: SLoginPluginResponse,
    ) -> Result<(GameProfile, SocketAddr), LoginError> {
        log::debug!("Handling plugin");
        let velocity_config = &advanced_config().networking.proxy.velocity;
        if velocity_config.enabled {
            Ok(velocity::receive_velocity_plugin_response(
                self.address.port(),
                velocity_config,
                plugin_response,
            )?)
        } else {
            Err(LoginError::IgnoredPluginRequest)
        }
    }

    pub async fn handle_login_acknowledged(&self, server: &Server) {
        log::debug!("Handling login acknowledgement");
        self.connection_state = ConnectionState::Config;
        self.send_packet_now(&server.get_branding()).await;

        if advanced_config().server_links.enabled {
            self.send_packet_now(&CConfigServerLinks::new(&LINKS)).await;
        }

        // Send tags.
        // TODO: Is this the right place to send them?

        self.send_packet_now(&CUpdateTags::new(&[
            pumpkin_data::tag::RegistryKey::Block,
            pumpkin_data::tag::RegistryKey::Fluid,
            pumpkin_data::tag::RegistryKey::Enchantment,
            pumpkin_data::tag::RegistryKey::WorldgenBiome,
            pumpkin_data::tag::RegistryKey::Item,
            pumpkin_data::tag::RegistryKey::EntityType,
        ]))
        .await;

        let resource_config = &advanced_config().resource_pack;
        if resource_config.enabled {
            let uuid = Uuid::new_v3(&uuid::Uuid::NAMESPACE_DNS, resource_config.url.as_bytes());
            let resource_pack = CConfigAddResourcePack::new(
                &uuid,
                &resource_config.url,
                &resource_config.sha1,
                resource_config.force,
                if resource_config.prompt_message.is_empty() {
                    None
                } else {
                    Some(TextComponent::text(&resource_config.prompt_message))
                },
            );

            self.send_packet_now(&resource_pack).await;
        } else {
            // This will be invoked by our resource pack handler in the case of the above branch.
            self.send_known_packs().await;
        }
        log::debug!("login acknowledged");
    }

    /// Send the known data packs to the client.
    pub async fn send_known_packs(&self) {
        self.send_packet_now(&CKnownPacks::new(&[KnownPack {
            namespace: "minecraft",
            id: "core",
            version: "1.21.8",
        }]))
        .await;
    }
}

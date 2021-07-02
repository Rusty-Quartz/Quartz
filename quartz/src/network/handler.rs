use super::AsyncWriteHandle;
use crate::{
    command::{CommandContext, CommandSender},
    command_executor,
    config,
    display_to_console,
    network::{packet::*, AsyncClientConnection, ConnectionState, PacketBuffer},
    server::{self, QuartzServer},
    world::location::BlockPosition,
};
use hex::ToHex;
use lazy_static::lazy_static;
use log::{debug, error};
use openssl::{
    pkey::Private,
    rsa::{Padding, Rsa},
    sha,
};
use quartz_chat::{color::PredefinedColor, Component};
use quartz_commands::CommandModule;
use quartz_util::UnlocalizedName;
use rand::{thread_rng, Rng};
use regex::Regex;
use serde::Deserialize;
use serde_json::json;
use std::{
    str::FromStr,
    sync::{mpsc::Sender, Arc},
};
use uuid::Uuid;

/// The numeric protocol version the server uses.
pub const PROTOCOL_VERSION: i32 = 755;
/// The ID for the legacy ping packet.
pub const LEGACY_PING_PACKET_ID: i32 = 0xFE;

include!(concat!(env!("OUT_DIR"), "/packet_handler_output.rs"));

pub(crate) struct AsyncPacketHandler {
    key_pair: Arc<Rsa<Private>>,
    username: String,
    verify_token: Vec<u8>,
}

impl AsyncPacketHandler {
    fn new(key_pair: Arc<Rsa<Private>>) -> Self {
        AsyncPacketHandler {
            key_pair,
            username: String::new(),
            verify_token: Vec::new(),
        }
    }
}

impl AsyncPacketHandler {
    async fn handle_handshake(
        &mut self,
        conn: &mut AsyncClientConnection,
        version: i32,
        next_state: i32,
    ) {
        if version != PROTOCOL_VERSION {
            conn.connection_state = ConnectionState::Disconnected;
            return;
        }

        if next_state == 1 {
            conn.connection_state = ConnectionState::Status;
        } else if next_state == 2 {
            conn.connection_state = ConnectionState::Login;
        }
    }

    async fn handle_ping(&mut self, conn: &mut AsyncClientConnection, payload: i64) {
        conn.send_packet(&ClientBoundPacket::Pong { payload }).await;
    }

    async fn handle_login_start(&mut self, conn: &mut AsyncClientConnection, name: &String) {
        // Store username for later
        self.username = name.to_owned();

        // Generate and store verify token
        let mut verify_token = [0_u8; 4];
        thread_rng().fill(&mut verify_token);
        self.verify_token = verify_token.to_vec();

        // Format public key to send to client
        let pub_key_der;
        match self.key_pair.public_key_to_der() {
            Ok(der) => pub_key_der = der,
            Err(e) => {
                error!("Failed to convert public key to der: {}", e);
                conn.shutdown();
                return;
            }
        }

        conn.send_packet(&ClientBoundPacket::EncryptionRequest {
            server_id: "".to_owned(),
            public_key_length: pub_key_der.len() as i32,
            public_key: pub_key_der,
            verify_token_length: verify_token.len() as i32,
            verify_token: verify_token.to_vec(),
        })
        .await;
    }

    async fn handle_encryption_response(
        &mut self,
        conn: &mut AsyncClientConnection,
        shared_secret: &Vec<u8>,
        verify_token: &Vec<u8>,
    ) {
        // Decrypt and check verify token
        let mut decrypted_verify = vec![0; self.key_pair.size() as usize];
        if let Err(e) =
            self.key_pair
                .private_decrypt(verify_token, &mut decrypted_verify, Padding::PKCS1)
        {
            error!("Failed to decrypt verify token: {}", e);
            conn.shutdown();
            return;
        }
        decrypted_verify = decrypted_verify[.. self.verify_token.len()].to_vec();

        if self.verify_token != decrypted_verify {
            error!(
                "verify for client {} didn't match, {:x?}, {:x?}",
                conn.id, self.verify_token, decrypted_verify
            );
            return conn
                .send_packet(&ClientBoundPacket::Disconnect {
                    reason: Component::colored(
                        "Error verifying encryption".to_owned(),
                        PredefinedColor::Red,
                    ),
                })
                .await;
        }

        // Decrypt shared secret
        let mut decrypted_secret = vec![0; self.key_pair.size() as usize];
        if let Err(e) =
            self.key_pair
                .private_decrypt(shared_secret, &mut decrypted_secret, Padding::PKCS1)
        {
            error!("Failed to decrypt secret key: {}", e);
            conn.shutdown();
            return;
        }
        decrypted_secret = decrypted_secret[.. 16].to_vec();

        // Initiate encryption
        if let Err(e) = conn.initiate_encryption(decrypted_secret.as_slice()).await {
            error!(
                "Failed to initialize encryption for client connetion: {}",
                e
            );
            conn.shutdown();
            return;
        }

        // Generate server id hash
        let mut hasher = sha::Sha1::new();

        hasher.update(decrypted_secret.as_slice());
        match self.key_pair.public_key_to_der() {
            Ok(der) => hasher.update(&*der),
            Err(e) => {
                error!("Failed to convert public key to der: {}", e);
                conn.shutdown();
                return;
            }
        }

        let mut hash = hasher.finish();
        let hash_hex;

        // Big thanks to https://gist.github.com/RoccoDev/8fa130f1946f89702f799f89b8469bc9 for writing this minecraft hashing code
        lazy_static! {
            static ref LEADING_ZERO_REGEX: Regex = Regex::new(r#"^0+"#).unwrap();
        }

        let negative = (hash[0] & 0x80) == 0x80;

        if negative {
            let mut carry = true;
            for i in (0 .. hash.len()).rev() {
                hash[i] = !hash[i] & 0xff;
                if carry {
                    carry = hash[i] == 0xff;
                    hash[i] = hash[i] + 1;
                }
            }

            hash_hex = format!(
                "-{}",
                LEADING_ZERO_REGEX.replace(&hash.encode_hex::<String>(), "")
            );
        } else {
            hash_hex = LEADING_ZERO_REGEX
                .replace(&hash.encode_hex::<String>(), "")
                .to_string();
        }

        // use hash and username to generate link to mojang's servers
        // TODO: Implement prevent-proxy-connections by adding client ip to post req
        let url = format!(
            "https://sessionserver.mojang.com/session/minecraft/hasJoined?username={}&serverId={}",
            &self.username, &hash_hex
        );

        // Structs used to allow serde to parse response json into struct
        #[derive(Deserialize)]
        #[allow(unused)]
        struct Properties {
            name: String,
            value: String,
            signature: String,
        }

        #[derive(Deserialize)]
        #[allow(unused)]
        struct AuthResponse {
            id: String,
            name: String,
            properties: [Properties; 1],
        }

        // TODO: Currently disabled cause no need rn, will enable via config later
        // conn.send_packet(&ClientBoundPacket::SetCompression{threshhold: /* maximum size of uncompressed packet */})

        // Make a get request
        let mojang_req = ureq::get(&url).call();
        let string_uuid = match mojang_req.map(|response| response.into_json::<AuthResponse>()) {
            Ok(Ok(AuthResponse { id, .. })) => id,
            Ok(Err(e)) => {
                error!("Failed to parse response JSON: {}", e);
                return;
            }
            Err(e) => {
                error!("Failed to parse authentication response: {}", e);
                return;
            }
        };

        match Uuid::from_str(&string_uuid) {
            Ok(uuid) => {
                conn.send_packet(&ClientBoundPacket::LoginSuccess {
                    uuid,
                    username: self.username.clone(),
                })
                .await;
            }
            Err(e) => error!("Failed to parse malformed UUID: {}", e),
        }
    }

    async fn handle_login_plugin_response(
        &mut self,
        _conn: &mut AsyncClientConnection,
        _message_id: i32,
        _successful: bool,
        _data: &Option<Vec<u8>>,
    ) {
        // TODO: Implement login_plugin_response
    }
}

impl QuartzServer {
    async fn handle_login_success_server(&mut self, _sender: usize, _uuid: Uuid, _username: &str) {
        // TODO: Implement login_success_server
    }

    async fn handle_console_command(&mut self, command: &str) {
        let executor = command_executor();
        let sender = CommandSender::Console;
        let context = CommandContext::new(self, &*executor, sender);
        if let Err(e) = executor.dispatch(command, context) {
            display_to_console(&e);
        }
    }

    async fn handle_console_completion(&mut self, command: &str, response: &Sender<Vec<String>>) {
        let executor = command_executor();
        let sender = CommandSender::Console;
        let context = CommandContext::new(self, &*executor, sender);
        let suggestions = executor.get_suggestions(command, &context);
        // Error handling not useful here
        drop(response.send(suggestions));
    }

    async fn handle_legacy_server_list_ping(&mut self, sender: usize, _payload: u8) {
        // Load in all needed values from server object
        let protocol_version = u16::to_string(&(PROTOCOL_VERSION as u16));
        let version = server::VERSION;
        let player_count = self.client_list.online_count().to_string();
        let config = config().lock().await;
        let motd = &config.motd;
        let max_players = config.max_players.to_string();

        // Add String header
        let mut string_vec: Vec<u16> = vec![0x00A7, 0x0031, 0x0000];

        // Add all fields to vector
        string_vec.extend(
            protocol_version
                .chars()
                .rev()
                .collect::<String>()
                .encode_utf16(),
        );
        string_vec.push(0x0000);

        string_vec.extend(version.encode_utf16());
        string_vec.push(0x0000);

        string_vec.extend(motd.as_plain_text().encode_utf16());
        string_vec.push(0x0000);

        string_vec.extend(player_count.encode_utf16());
        string_vec.push(0x0000);

        string_vec.extend(max_players.encode_utf16());

        let mut buffer = PacketBuffer::new(3 + string_vec.len());

        // Write FF and length
        buffer.write_bytes(&[0xFF]);
        buffer.write(&(string_vec.len() as u16));

        // Write String
        for bytes in string_vec {
            buffer.write(&bytes);
        }

        self.client_list.send_buffer(sender, buffer).await;
    }

    async fn handle_status_request(&mut self, sender: usize) {
        let config = config().lock().await;
        let json_response = json!({
            "version": {
                "name": server::VERSION,
                "protocol": PROTOCOL_VERSION
            },
            "players": {
                "max": config.max_players,
                "online": self.client_list.online_count(),
                "sample": [] // TODO: Decide whether or not to implement "sample" in status req
            },
            "description": config.motd
        });

        // TODO: implement favicon

        self.client_list
            .send_packet(sender, ClientBoundPacket::StatusResponse {
                json_response: json_response.to_string(),
            })
            .await;
    }

    #[allow(unused_variables)]
    async fn handle_client_disconnected(&mut self, id: usize) {}

    #[allow(unused_variables)]
    async fn handle_client_connected(&mut self, id: usize, write_handle: &AsyncWriteHandle) {}

    #[allow(unused_variables)]
    async fn handle_use_item(&mut self, sender: usize, hand: i32) {}

    #[allow(unused_variables)]
    async fn handle_player_block_placement(
        &mut self,
        sender: usize,
        hand: i32,
        location: &BlockPosition,
        face: i32,
        cursor_position_x: f32,
        cursor_position_y: f32,
        cursor_position_z: f32,
        inside_block: bool,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_spectate(&mut self, sender: usize, target_player: Uuid) {}

    #[allow(unused_variables)]
    async fn handle_animation(&mut self, sender: usize, hand: i32) {}

    #[allow(unused_variables)]
    async fn handle_update_sign(
        &mut self,
        sender: usize,
        location: &BlockPosition,
        line_1: &str,
        line_2: &str,
        line_3: &str,
        line_4: &String,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_update_structure_block(
        &mut self,
        sender: usize,
        location: &BlockPosition,
        action: i32,
        mode: i32,
        name: &String,
        offset_x: i8,
        offset_y: i8,
        offset_z: i8,
        size_x: i8,
        size_y: i8,
        size_z: i8,
        mirror: i32,
        rotation: i32,
        metadate: &str,
        integrity: f32,
        seed: i64,
        flags: i8,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_creative_inventory_action(
        &mut self,
        sender: usize,
        slot: i16,
        clicked_item: &Slot,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_update_jigsaw_block(
        &mut self,
        sender: usize,
        location: &BlockPosition,
        name: &UnlocalizedName,
        target: &UnlocalizedName,
        pool: &UnlocalizedName,
        final_state: &str,
        joint_type: &str,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_update_command_block_minecart(
        &mut self,
        sender: usize,
        entity_id: i32,
        command: &str,
        track_output: bool,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_update_command_block(
        &mut self,
        sender: usize,
        location: &BlockPosition,
        command: &str,
        mode: i32,
        flags: i8,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_held_item_change(&mut self, sender: usize, slot: i16) {}

    #[allow(unused_variables)]
    async fn handle_set_beacon_effect(
        &mut self,
        sender: usize,
        primary_effect: i32,
        secondary_effect: i32,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_select_trade(&mut self, sender: usize, selected_slod: i32) {}

    #[allow(unused_variables)]
    async fn handle_advancement_tab(
        &mut self,
        sender: usize,
        action: i32,
        tab_id: &Option<UnlocalizedName>,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_resource_pack_status(&mut self, sender: usize, result: i32) {}

    #[allow(unused_variables)]
    async fn handle_name_item(&mut self, sender: usize, item_name: &str) {}

    #[allow(unused_variables)]
    async fn handle_set_recipe_book_state(
        &mut self,
        sender: usize,
        book_id: i32,
        book_open: bool,
        filter_active: bool,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_set_displayed_recipe(&mut self, sender: usize, recipe_id: &UnlocalizedName) {}

    #[allow(unused_variables)]
    async fn handle_steer_vehicle(
        &mut self,
        sender: usize,
        sideways: f32,
        forward: f32,
        flags: u8,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_pong(&mut self, sender: usize, id: i32) {}

    #[allow(unused_variables)]
    async fn handle_entity_action(
        &mut self,
        sender: usize,
        entity_id: i32,
        action_id: i32,
        jump_boost: i32,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_player_digging(
        &mut self,
        sender: usize,
        status: i32,
        location: &BlockPosition,
        face: i8,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_player_abilities(&mut self, sender: usize, flags: i8) {}

    #[allow(unused_variables)]
    async fn handle_craft_recipe_request(
        &mut self,
        sender: usize,
        window_id: i8,
        recipe: &UnlocalizedName,
        make_all: bool,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_pick_item(&mut self, sender: usize, slot_to_use: i32) {}

    #[allow(unused_variables)]
    async fn handle_steer_boat(
        &mut self,
        sender: usize,
        left_paddle_turning: bool,
        right_paddle_turning: bool,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_player_movement(&mut self, sender: usize, on_ground: bool) {}

    #[allow(unused_variables)]
    async fn handle_player_rotation(
        &mut self,
        sender: usize,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_vehicle_move(
        &mut self,
        sender: usize,
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_player_position(
        &mut self,
        sender: usize,
        x: f64,
        feet_y: f64,
        z: f64,
        on_ground: bool,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_player_position_and_rotation(
        &mut self,
        sender: usize,
        x: f64,
        feet_y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_lock_difficulty(&mut self, sender: usize, locked: bool) {}

    #[allow(unused_variables)]
    async fn handle_keep_alive(&mut self, sender: usize, keep_alive_id: i64) {}

    #[allow(unused_variables)]
    async fn handle_generate_structure(
        &mut self,
        sender: usize,
        location: &BlockPosition,
        levels: i32,
        keep_jigsaws: bool,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_interact_entity(
        &mut self,
        sender: usize,
        entity_id: i32,
        r#type: i32,
        target_x: Option<f32>,
        target_y: Option<f32>,
        target_z: Option<f32>,
        hand: Option<i32>,
        sneaking: bool,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_edit_book(
        &mut self,
        sender: usize,
        new_book: &Slot,
        is_signing: bool,
        hand: i32,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_plugin_message(
        &mut self,
        sender: usize,
        channel: &UnlocalizedName,
        data: &Vec<u8>,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_close_window(&mut self, sender: usize, window_id: u8) {}

    #[allow(unused_variables)]
    async fn handle_click_window(
        &mut self,
        sender: usize,
        window_id: u8,
        slot: i16,
        button: i8,
        mode: i32,
        slots_len: i32,
        slots: &Vec<Slot>,
        clicked_slot: &Slot,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_click_window_button(&mut self, sender: usize, window_id: i8, button_id: i8) {}

    #[allow(unused_variables)]
    async fn handle_tab_complete(&mut self, sender: usize, trasaction_id: i32, text: &str) {}

    #[allow(unused_variables)]
    async fn handle_client_settings(
        &mut self,
        sender: usize,
        locale: &str,
        view_distance: i8,
        chat_mode: i32,
        chat_colors: bool,
        displayed_skin_parts: u8,
        main_hand: i32,
        disable_text_filtering: bool,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_client_status(&mut self, sender: usize, action_id: i32) {}

    #[allow(unused_variables)]
    async fn handle_chat_message(&mut self, sender: usize, messag: &str) {}

    #[allow(unused_variables)]
    async fn handle_set_difficulty(&mut self, sender: usize, new_difficulty: i8) {}

    #[allow(unused_variables)]
    async fn handle_query_entity_nbt(&mut self, sender: usize, trasaction_id: i32, entity_id: i32) {
    }

    #[allow(unused_variables)]
    async fn handle_query_block_nbt(
        &mut self,
        sender: usize,
        trasaction_id: i32,
        location: &BlockPosition,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_teleport_confirm(&mut self, sender: usize, teleport_id: i32) {}
}

/// Handles the given asynchronos connecting using blocking I/O opperations.
pub async fn handle_async_connection(
    mut conn: AsyncClientConnection,
    private_key: Arc<Rsa<Private>>,
) {
    let mut async_handler = AsyncPacketHandler::new(private_key);

    while conn.connection_state != ConnectionState::Disconnected {
        match conn.read_packet().await {
            Ok(packet_len) => {
                // Client disconnected
                if packet_len == 0 {
                    break;
                }
                // Handle the packet
                else {
                    if let Err(e) = handle_packet(&mut conn, &mut async_handler, packet_len).await {
                        error!("Failed to handle packet: {}", e);
                        conn.shutdown();
                        break;
                    }
                }
            }
            Err(e) => {
                error!("Error in connection handler: {}", e);
                conn.shutdown();
                break;
            }
        }
    }

    conn.forward_to_server(ServerBoundPacket::ClientDisconnected { id: conn.id });
    debug!("Client disconnected");
}
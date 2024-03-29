use crate::{
    command::{CommandContext, CommandSender},
    command_executor,
    config,
    entities::{
        player::{Player, PlayerInventory, PlayerState},
        Position,
    },
    item::{ItemStack, EMPTY_ITEM_STACK},
    network::{packet_data::*, *},
    server::{self, ClientId, QuartzServer},
    world::world::Dimension,
};
use qdat::{
    world::location::{BlockFace, BlockPosition, Coordinate},
    Gamemode,
};

use hex::ToHex;
use log::{debug, error};
use once_cell::sync::Lazy;
use openssl::{
    pkey::Private,
    rsa::{Padding, Rsa},
    sha,
};
use qdat::UnlocalizedName;
use quartz_chat::{color::Color, Component};
use quartz_commands::CommandModule;
use quartz_nbt::NbtCompound;
use rand::{thread_rng, Rng};
use regex::Regex;
use serde::Deserialize;
use serde_json::json;
use std::{str::FromStr, sync::Arc, time::Instant};
use uuid::Uuid;

mod build {
    #![allow(clippy::redundant_pattern, clippy::undocumented_unsafe_blocks)]
    use super::*;
    include!(concat!(env!("OUT_DIR"), "/packet_handler_output.rs"));
}
pub use build::*;


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
        conn.write_handle
            .send_packet(ClientBoundPacket::Pong { payload });
    }

    async fn handle_login_start(&mut self, conn: &mut AsyncClientConnection, name: &str) {
        // If we are not running in online mode we just send LoginSuccess and skip encryption
        if !config().read().online_mode {
            conn.write_handle
                .send_packet(ClientBoundPacket::LoginSuccess {
                    uuid: Uuid::from_u128(0),
                    username: name.to_owned(),
                });

            conn.forward_internal_to_server(WrappedServerBoundPacket::LoginSuccess {
                id: conn.id,
                uuid: Uuid::from_u128(0),
                username: name.to_owned(),
            });

            conn.connection_state = ConnectionState::Play;

            return;
        }

        // Store username for later
        self.username = name.to_owned();

        // Generate and store verify token
        let mut verify_token = [0_u8; 4];
        thread_rng().fill(&mut verify_token);
        self.verify_token = verify_token.to_vec();

        // Format public key to send to client
        let pub_key_der = match self.key_pair.public_key_to_der() {
            Ok(der) => der,
            Err(e) => {
                error!("Failed to convert public key to der: {}", e);
                conn.write_handle.shutdown();
                return;
            }
        };

        conn.write_handle
            .send_packet(ClientBoundPacket::EncryptionRequest {
                server_id: "".to_owned(),
                public_key: pub_key_der.into_boxed_slice(),
                verify_token: verify_token.to_vec().into_boxed_slice(),
            });
    }

    async fn handle_encryption_response(
        &mut self,
        conn: &mut AsyncClientConnection,
        shared_secret: &[u8],
        verify_token: &[u8],
    ) {
        // Decrypt and check verify token
        let mut decrypted_verify = vec![0; self.key_pair.size() as usize];
        if let Err(e) =
            self.key_pair
                .private_decrypt(verify_token, &mut decrypted_verify, Padding::PKCS1)
        {
            error!("Failed to decrypt verify token: {}", e);
            conn.write_handle.shutdown();
            return;
        }
        decrypted_verify = decrypted_verify[.. self.verify_token.len()].to_vec();

        if self.verify_token != decrypted_verify {
            error!(
                "verify for client {} didn't match, {:x?}, {:x?}",
                conn.id, self.verify_token, decrypted_verify
            );
            return conn
                .write_handle
                .send_packet(ClientBoundPacket::Disconnect {
                    reason: Box::new(Component::colored(
                        "Error verifying encryption".to_owned(),
                        Color::Red,
                    )),
                });
        }

        // Decrypt shared secret
        let mut decrypted_secret = vec![0; self.key_pair.size() as usize];
        if let Err(e) =
            self.key_pair
                .private_decrypt(shared_secret, &mut decrypted_secret, Padding::PKCS1)
        {
            error!("Failed to decrypt secret key: {}", e);
            conn.write_handle.shutdown();
            return;
        }
        decrypted_secret = decrypted_secret[.. 16].to_vec();

        // Initiate encryption
        let init_encryption_result = conn.initiate_encryption(decrypted_secret.as_slice());
        if let Err(e) = init_encryption_result {
            error!(
                "Failed to initialize encryption for client connetion: {}",
                e
            );
            conn.write_handle.shutdown();
            return;
        }

        // Generate server id hash
        let mut hasher = sha::Sha1::new();

        hasher.update(decrypted_secret.as_slice());
        match self.key_pair.public_key_to_der() {
            Ok(der) => hasher.update(&*der),
            Err(e) => {
                error!("Failed to convert public key to der: {}", e);
                conn.write_handle.shutdown();
                return;
            }
        }

        let mut hash = hasher.finish();
        let hash_hex;

        // Big thanks to https://gist.github.com/RoccoDev/8fa130f1946f89702f799f89b8469bc9 for writing this minecraft hashing code
        static LEADING_ZERO_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r#"^0+"#).unwrap());

        let negative = (hash[0] & 0x80) == 0x80;

        if negative {
            let mut carry = true;
            for i in (0 .. hash.len()).rev() {
                hash[i] = !hash[i];
                if carry {
                    carry = hash[i] == 0xff;
                    hash[i] += 1;
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

        // TODO: enable compression properly here
        const TEST_THRESHOLD: i32 = 0;
        conn.write_handle
            .send_packet(WrappedClientBoundPacket::EnableCompression {
                threshold: TEST_THRESHOLD,
            });

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
                conn.write_handle
                    .send_packet(ClientBoundPacket::LoginSuccess {
                        uuid,
                        username: self.username.clone(),
                    });

                conn.connection_state = ConnectionState::Play;

                conn.forward_internal_to_server(WrappedServerBoundPacket::LoginSuccess {
                    id: conn.id,
                    uuid,
                    username: self.username.clone(),
                });
            }
            Err(e) => error!("Failed to parse malformed UUID: {}", e),
        }
    }

    async fn handle_login_plugin_response(
        &mut self,
        _conn: &mut AsyncClientConnection,
        _message_id: i32,
        _data: &Option<Box<[u8]>>,
    ) {
        // TODO: Implement login_plugin_response
    }
}

impl QuartzServer {
    pub(crate) async fn handle_login_success_server(
        &mut self,
        sender: ClientId,
        mut uuid: Uuid,
        username: &str,
    ) {
        let config = config().read();
        if !config.online_mode {
            // I have no idea what to use as the namespace here lol
            uuid = Uuid::new_v4()
            // Use a random namespace so we can have multiple players with the same username
            //     &Uuid::from_str("OfflinePlayer").unwrap(),
            //     username.as_bytes(),
            // );
        }
        self.client_list.set_username(sender, username);
        log::debug!("{}", uuid);
        self.client_list.set_uuid(sender, uuid);

        /*

            {
                logical_height:256S,
                coordinate_scale:1F,
                natural:1B,
                ultrawarm:0B,
                ambient_light:0F,
                respawn_anchor_works:0B,
                infiniburn:"minecraft:infiniburn_overworld",
                effects:"minecraft:overworld",
                has_ceiling:0B,
                bed_works:1B,
                has_skylight:1B,
                piglin_safe:0B,
                has_raids:1B
            }

        */
        let dimension =
            match quartz_nbt::snbt::parse(include_str!("../../../assets/dimension.snbt")) {
                Ok(nbt) => Box::new(nbt),
                Err(e) => {
                    error!("Error in dimension snbt: {}", e);
                    Box::new(NbtCompound::new())
                }
            };

        let dimension_codec =
            match quartz_nbt::snbt::parse(include_str!("../../../assets/dimension_codec.snbt")) {
                Ok(nbt) => Box::new(nbt),
                Err(e) => {
                    error!("Error in dimension codec snbt: {}", e);
                    Box::new(NbtCompound::new())
                }
            };

        self.client_list
            .send_packet(sender, ClientBoundPacket::JoinGame {
                entity_id: 0,
                is_hardcore: false,
                gamemode: Gamemode::Creative,
                previous_gamemode: Gamemode::None,
                world_names: vec![UnlocalizedName::minecraft("overworld")].into_boxed_slice(),
                dimension_codec,
                dimension,
                world_name: UnlocalizedName::minecraft("overworld"),
                hashed_seed: 0,
                max_players: 10,
                view_distance: 12,
                reduced_debug_info: false,
                enable_respawn_screen: true,
                is_debug: false,
                is_flat: false,
            });

        let mut brand_buf = PacketBuffer::new(2048);
        brand_buf.write(&"Quartz");

        self.client_list
            .send_packet(sender, ClientBoundPacket::PluginMessage {
                channel: UnlocalizedName::minecraft("brand"),
                data: brand_buf[..].to_vec().into_boxed_slice(),
            });

        // Since at this point keep_alive on the AsyncPackeHandler is still -1 it won't check what this id is
        // So it doesn't matter if we hard code the id
        self.client_list.start_keep_alive(sender);
    }

    async fn handle_legacy_server_list_ping(&mut self, sender: ClientId, _payload: u8) {
        // Load in all needed values from server object
        let protocol_version = u16::to_string(&(PROTOCOL_VERSION as u16));
        let version = server::VERSION;
        let player_count = self.client_list.online_count().to_string();
        let config = config().read();
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
        buffer.write_bytes([0xFF]);
        buffer.write(&(string_vec.len() as u16));

        // Write String
        for bytes in string_vec {
            buffer.write(&bytes);
        }

        self.client_list.send_buffer(sender, buffer);
    }

    async fn handle_status_request(&mut self, sender: ClientId) {
        let config = config().read();

        #[derive(serde::Serialize, serde::Deserialize)]
        struct ClientSampleEntry<'a> {
            id: Uuid,
            name: &'a str,
        }

        let sample = self
            .client_list
            .iter()
            .enumerate()
            .filter_map(|(n, (_, client))| {
                if n < 5 {
                    Some(ClientSampleEntry {
                        name: client.username(),
                        id: *client.uuid(),
                    })
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let json_response = json!({
            "version": {
                "name": server::VERSION,
                "protocol": PROTOCOL_VERSION
            },
            "players": {
                "max": config.max_players,
                "online": self.client_list.online_count(),
                "sample": sample
            },
            "description": config.motd
        });

        // TODO: implement favicon

        self.client_list
            .send_packet(sender, ClientBoundPacket::StatusResponse {
                json_response: json_response.to_string(),
            });
    }

    async fn handle_keep_alive(&mut self, sender: ClientId, keep_alive_id: i64) {
        self.client_list.handle_keep_alive(sender, keep_alive_id);
    }

    #[allow(unused_variables)]
    async fn handle_use_item(&mut self, sender: ClientId, hand: i32) {}

    #[allow(unused_variables)]
    #[allow(clippy::too_many_arguments)]
    async fn handle_player_block_placement(
        &mut self,
        sender: ClientId,
        hand: i32,
        location: &BlockPosition,
        face: &BlockFace,
        cursor_position_x: f32,
        cursor_position_y: f32,
        cursor_position_z: f32,
        inside_block: bool,
    ) {
        let world = self.world_store.get_player_world_mut(sender).unwrap();
        let curr_item = {
            let player_entity = *world.get_player_entity(sender).unwrap();
            let entities = world.get_entities().await;
            let player_inv = entities.get::<PlayerInventory>(player_entity).unwrap();
            player_inv.current_slot()
        };

        if let Some(i) = curr_item.item() {
            let mut chunk = world.get_loaded_chunk_mut((*location).into()).unwrap();
            // TODO: change this over to using block items to allow conversions between items and blocks
            if let Some(s) = qdat::item::item_to_block(i.item) {
                let offset_pos = (*location).face_offset(face);
                let last_state = chunk.set_block_state_at(offset_pos, s.id());
                self.client_list
                    .send_to_all(|_| ClientBoundPacket::BlockChange {
                        location: offset_pos,
                        block_id: s.id() as i32,
                    });
            };
        }
    }

    #[allow(unused_variables)]
    async fn handle_spectate(&mut self, sender: ClientId, target_player: Uuid) {}

    #[allow(unused_variables)]
    async fn handle_animation(&mut self, sender: ClientId, hand: i32) {}

    #[allow(unused_variables)]
    async fn handle_update_sign(
        &mut self,
        sender: ClientId,
        location: &BlockPosition,
        line_1: &str,
        line_2: &str,
        line_3: &str,
        line_4: &str,
    ) {
    }

    #[allow(unused_variables)]
    #[allow(clippy::too_many_arguments)]
    async fn handle_update_structure_block(
        &mut self,
        sender: ClientId,
        location: &BlockPosition,
        action: i32,
        mode: i32,
        name: &str,
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
        sender: ClientId,
        slot: i16,
        clicked_item: &Slot,
    ) {
        if slot != -1 {
            let world = self.world_store.get_player_world_mut(sender).unwrap();
            let player_entity = *world.get_player_entity(sender).unwrap();
            let entities = world.get_entities_mut().await;
            let mut player_inv = entities.get_mut::<PlayerInventory>(player_entity).unwrap();

            if clicked_item.present {
                player_inv.set_slot(
                    slot as usize,
                    ItemStack::new(
                        qdat::item::ITEM_LOOKUP_BY_NUMERIC_ID
                            .get(&clicked_item.item_id.unwrap())
                            .unwrap(),
                    )
                    .into(),
                );
            } else {
                player_inv.set_slot(slot as usize, EMPTY_ITEM_STACK);
            }

            self.client_list
                .send_packet(sender, ClientBoundPacket::SetSlot {
                    window_id: 0,
                    slot,
                    slot_data: clicked_item.clone(),
                });
        }
    }

    #[allow(unused_variables)]
    async fn handle_update_jigsaw_block(
        &mut self,
        sender: ClientId,
        location: &BlockPosition,
        data: &JigsawUpdateData,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_update_command_block_minecart(
        &mut self,
        sender: ClientId,
        entity_id: i32,
        command: &str,
        track_output: bool,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_update_command_block(
        &mut self,
        sender: ClientId,
        location: &BlockPosition,
        command: &str,
        mode: i32,
        flags: i8,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_held_item_change(&mut self, sender: ClientId, slot: i16) {
        let world = self.world_store.get_player_world_mut(sender).unwrap();
        let player_entity = *world.get_player_entity(sender).unwrap();
        world
            .get_entities_mut()
            .await
            .get_mut::<PlayerInventory>(player_entity)
            .unwrap()
            .set_curr_slot(slot as u8 + 36);
        self.client_list
            .send_packet(sender, ClientBoundPacket::HeldItemChange {
                slot: slot as i8,
            })
    }

    #[allow(unused_variables)]
    async fn handle_set_beacon_effect(
        &mut self,
        sender: ClientId,
        primary_effect: i32,
        secondary_effect: i32,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_select_trade(&mut self, sender: ClientId, selected_slod: i32) {}

    #[allow(unused_variables)]
    async fn handle_advancement_tab(
        &mut self,
        sender: ClientId,
        action: i32,
        tab_id: &Option<UnlocalizedName>,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_resource_pack_status(&mut self, sender: ClientId, result: i32) {}

    #[allow(unused_variables)]
    async fn handle_name_item(&mut self, sender: ClientId, item_name: &str) {}

    #[allow(unused_variables)]
    async fn handle_set_recipe_book_state(
        &mut self,
        sender: ClientId,
        book_id: i32,
        book_open: bool,
        filter_active: bool,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_set_displayed_recipe(&mut self, sender: ClientId, recipe_id: &UnlocalizedName) {
    }

    #[allow(unused_variables)]
    async fn handle_steer_vehicle(
        &mut self,
        sender: ClientId,
        sideways: f32,
        forward: f32,
        flags: u8,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_pong(&mut self, sender: ClientId, id: i32) {}

    #[allow(unused_variables)]
    async fn handle_entity_action(
        &mut self,
        sender: ClientId,
        entity_id: i32,
        action_id: i32,
        jump_boost: i32,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_player_digging(
        &mut self,
        sender: ClientId,
        status: i32,
        location: &BlockPosition,
        face: &BlockFace,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_player_abilities(&mut self, sender: ClientId, flags: i8) {}

    #[allow(unused_variables)]
    async fn handle_craft_recipe_request(
        &mut self,
        sender: ClientId,
        window_id: i8,
        recipe: &UnlocalizedName,
        make_all: bool,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_pick_item(&mut self, sender: ClientId, slot_to_use: i32) {}

    #[allow(unused_variables)]
    async fn handle_steer_boat(
        &mut self,
        sender: ClientId,
        left_paddle_turning: bool,
        right_paddle_turning: bool,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_player_movement(&mut self, sender: ClientId, on_ground: bool) {}

    #[allow(unused_variables)]
    async fn handle_player_rotation(
        &mut self,
        sender: ClientId,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_vehicle_move(
        &mut self,
        sender: ClientId,
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
        sender: ClientId,
        x: f64,
        feet_y: f64,
        z: f64,
        on_ground: bool,
    ) {
        // We assume the player is in the game if we're getting this packet
        let world = self.world_store.get_player_world_mut(sender).unwrap();
        let player_entity = *world.get_player_entity(sender).unwrap();
        let mut entities = world.get_entities_mut().await;
        let uuid = *self.client_list.uuid(sender).unwrap();

        // If the player isn't ready we need to return
        match *entities.get::<PlayerState>(player_entity).unwrap() {
            PlayerState::Ready => {}
            _ => return,
        }

        let pos = entities.get::<Position>(player_entity).unwrap();
        let dx = (x * 32. - pos.x * 32.) * 128.;
        let dy = (feet_y * 32. - pos.y * 32.) * 128.;
        let dz = (z * 32. - pos.z * 32.) * 128.;
        drop(pos);

        for (id, (pos, write_handle)) in
            entities.query_mut::<(&mut Position, &mut AsyncWriteHandle)>()
        {
            if id == player_entity {
                pos.x = x;
                pos.y = feet_y;
                pos.z = z;
            } else {
                write_handle.send_packet(ClientBoundPacket::EntityPosition {
                    entity_id: player_entity.id() as i32,
                    delta_x: dx as i16,
                    delta_y: dy as i16,
                    delta_z: dz as i16,
                    on_ground,
                })
            }
        }
    }

    #[allow(unused_variables)]
    #[allow(clippy::too_many_arguments)]
    async fn handle_player_position_and_rotation(
        &mut self,
        sender: ClientId,
        x: f64,
        feet_y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    ) {
        // We assume the player is in the game if we're getting this packet
        let world = self.world_store.get_player_world_mut(sender).unwrap();
        let player_entity = *world.get_player_entity(sender).unwrap();
        let mut entities = world.get_entities_mut().await;
        let uuid = *self.client_list.uuid(sender).unwrap();

        // If the player isn't ready we need to return
        match *entities.get::<PlayerState>(player_entity).unwrap() {
            PlayerState::Ready => {}
            _ => return,
        }

        let pos = entities.get::<Position>(player_entity).unwrap();
        let dx = (x * 32. - pos.x * 32.) * 128.;
        let dy = (feet_y * 32. - pos.y * 32.) * 128.;
        let dz = (z * 32. - pos.z * 32.) * 128.;
        drop(pos);

        // TODO: filter the query to just the players in render distance of the player
        for (id, (player,)) in entities.query_mut::<(&mut Player,)>() {
            if id == player_entity {
                let mut pos = &mut player.pos;
                pos.x = x;
                pos.y = feet_y;
                pos.z = z;
            } else {
                player
                    .write_handle
                    .send_packet(ClientBoundPacket::EntityPositionAndRotation {
                        entity_id: player_entity.id() as i32,
                        delta_x: dx as i16,
                        delta_y: dy as i16,
                        delta_z: dz as i16,
                        yaw: (yaw / 256.0) as u8,
                        pitch: (pitch / 256.0) as u8,
                        on_ground,
                    })
            }
        }
    }

    #[allow(unused_variables)]
    async fn handle_lock_difficulty(&mut self, sender: ClientId, locked: bool) {}

    #[allow(unused_variables)]
    async fn handle_generate_structure(
        &mut self,
        sender: ClientId,
        location: &BlockPosition,
        levels: i32,
        keep_jigsaws: bool,
    ) {
    }

    #[allow(unused_variables)]
    #[allow(clippy::too_many_arguments)]
    async fn handle_interact_entity(
        &mut self,
        sender: ClientId,
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
        sender: ClientId,
        new_book: &Slot,
        is_signing: bool,
        hand: i32,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_plugin_message(
        &mut self,
        sender: ClientId,
        channel: &UnlocalizedName,
        data: &[u8],
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_close_window(&mut self, sender: ClientId, window_id: u8) {}

    #[allow(unused_variables)]
    #[allow(clippy::too_many_arguments)]
    async fn handle_click_window(
        &mut self,
        sender: ClientId,
        window_id: u8,
        slot: i16,
        button: i8,
        mode: i32,
        slots: &[InventorySlot],
        clicked_slot: &Slot,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_click_window_button(&mut self, sender: ClientId, window_id: i8, button_id: i8) {
    }

    #[allow(unused_variables)]
    async fn handle_tab_complete(&mut self, sender: ClientId, trasaction_id: i32, text: &str) {}

    #[allow(unused_variables)]
    #[allow(clippy::too_many_arguments)]
    async fn handle_client_settings(
        &mut self,
        sender: ClientId,
        locale: &str,
        view_distance: i8,
        chat_mode: i32,
        chat_colors: bool,
        displayed_skin_parts: u8,
        main_hand: i32,
        disable_text_filtering: bool,
    ) {
        // Unwrap is safe because we're guarrenteed to have a uuid at this point
        let uuid = *self.client_list.uuid(sender).unwrap();

        self.client_list
            .send_packet(sender, ClientBoundPacket::HeldItemChange { slot: 0 });

        self.client_list
            .send_packet(sender, ClientBoundPacket::DeclareRecipes {
                recipes: vec![].into_boxed_slice(),
            });

        // self.client_list
        //     .send_packet(sender, ClientBoundPacket::Tags {
        //         tag_arr_len: 0,
        //         tag_arrays: Vec::new(),
        //     })
        //     .await;

        // self.client_list
        //     .send_packet(sender, ClientBoundPacket::DeclareCommands {
        //         count: 0,
        //         nodes: vec![],
        //         root_index: 0,
        //     })
        //     .await;

        self.client_list
            .send_packet(sender, ClientBoundPacket::UnlockRecipes {
                action: 0,
                crafting_recipe_book_open: false,
                crafting_recipe_book_filter_active: false,
                smelting_recipe_book_open: false,
                smelting_recipe_book_filter_active: false,
                smoker_recipe_book_filter_active: false,
                smoker_recipe_book_open: false,
                blast_furnace_recipe_book_open: false,
                blast_furnace_recipe_book_filter_active: false,
                recipe_ids_1: vec![].into_boxed_slice(),
                recipe_ids_2: Some(vec![].into_boxed_slice()),
            });

        self.client_list
            .send_to_all(|_| ClientBoundPacket::PlayerInfo {
                action: 0,
                player: vec![WrappedPlayerInfoAction {
                    uuid,
                    action: PlayerInfoAction::AddPlayer {
                        name: self.client_list.username(sender).unwrap().to_string(),
                        properties: vec![].into_boxed_slice(),
                        gamemode: Gamemode::Creative,
                        ping: 120,
                        display_name: None,
                    },
                }]
                .into_boxed_slice(),
            });

        self.client_list
            .send_packet(sender, ClientBoundPacket::PlayerInfo {
                action: 2,
                player: vec![WrappedPlayerInfoAction {
                    uuid,
                    action: PlayerInfoAction::UpdateLatency { ping: 12 },
                }]
                .into_boxed_slice(),
            });

        self.client_list
            .send_packet(sender, ClientBoundPacket::UpdateViewPosition {
                chunk_x: 0,
                chunk_z: 0,
            });

        let player = self
            .world_store
            .spawn_player(
                Dimension::Overworld,
                sender,
                Player::new(
                    Gamemode::Creative,
                    Position {
                        x: 0.,
                        y: 100.,
                        z: 0.,
                    },
                    self.client_list.create_write_handle(sender).unwrap(),
                ),
            )
            .await
            .unwrap();

        self.client_list.send_to_filtered(
            |_| ClientBoundPacket::SpawnPlayer {
                entity_id: player.id() as i32,
                player_uuid: uuid,
                x: 0.,
                y: 100.,
                z: 0.,
                pitch: 0,
                yaw: 0,
            },
            |client_id| **client_id != sender,
        );

        let write_handle = self.client_list.create_write_handle(sender).unwrap();
        let player_world = self.world_store.get_player_world_mut(sender).unwrap();
        let start = Instant::now();
        for x in -view_distance .. view_distance {
            for z in -view_distance .. view_distance {
                let coords = Coordinate::chunk(x as i32, z as i32);
                player_world.load_chunk(coords);
            }
        }
        player_world.join_pending().await;
        let elapsed = start.elapsed();
        log::info!("Chunk load time: {:?}", elapsed);

        let start = Instant::now();
        let vd = view_distance as u8 as usize;
        let mut packets = Vec::with_capacity(vd * vd);
        for x in -view_distance .. view_distance {
            for z in -view_distance .. view_distance {
                let chunk_coords = Coordinate::chunk(x as i32, z as i32);
                let chunk = match player_world.get_loaded_chunk(chunk_coords) {
                    Some(c) => c,
                    None => continue,
                };
                let (primary_bit_mask, section_data) = chunk.gen_client_section_data();

                packets.push(ClientBoundPacket::ChunkData {
                    chunk_x: chunk_coords.x(),
                    chunk_z: chunk_coords.z(),
                    primary_bit_mask,
                    heightmaps: chunk.get_heightmaps(),
                    biomes: Box::from(chunk.biomes()),
                    // TODO: send block entities for chunk when we support them
                    block_entities: vec![].into_boxed_slice(),
                    data: section_data,
                });

                let (sky_light_mask, empty_sky_light_mask, sky_light_arrays) =
                    chunk.gen_sky_lights();
                let (block_light_mask, empty_block_light_mask, block_light_arrays) =
                    chunk.gen_block_lights();

                packets.push(ClientBoundPacket::UpdateLight {
                    chunk_x: chunk_coords.x(),
                    chunk_z: chunk_coords.z(),
                    trust_edges: true,
                    sky_light_mask,
                    block_light_mask,
                    empty_sky_light_mask,
                    empty_block_light_mask,
                    sky_light_arrays,
                    block_light_arrays,
                });
            }
        }
        self.client_list.send_all(sender, packets);
        let elapsed = start.elapsed();
        log::info!("Chunk and light send time: {:?}", elapsed);

        self.client_list
            .send_packet(sender, ClientBoundPacket::SpawnPosition {
                location: BlockPosition { x: 0, y: 100, z: 0 },
                angle: 0.0,
            });

        self.client_list
            .send_packet(sender, ClientBoundPacket::PlayerPositionAndLook {
                dismount_vehicle: false,
                x: 0.0,
                y: 100.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                flags: 0,
                teleport_id: 1,
            });

        self.client_list
            .send_packet(sender, ClientBoundPacket::PlayerInfo {
                action: 0,
                player: self
                    .client_list
                    .iter()
                    .filter_map(|(client_id, client)| {
                        if *client_id != sender {
                            Some(WrappedPlayerInfoAction {
                                uuid: *client.uuid(),
                                action: PlayerInfoAction::AddPlayer {
                                    name: client.username().to_owned(),
                                    properties: vec![].into_boxed_slice(),
                                    gamemode: Gamemode::Creative,
                                    ping: 50,
                                    display_name: None,
                                },
                            })
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            });

        let overworld = self.world_store.get_world(Dimension::Overworld).unwrap();
        let entities = overworld.get_entities().await;

        let player_packets = overworld
            .get_players()
            .filter_map(|(client_id, player_id)| {
                if *client_id != sender {
                    let pos = entities.get::<Position>(*player_id).unwrap();
                    Some(ClientBoundPacket::SpawnPlayer {
                        entity_id: player_id.id() as i32,
                        player_uuid: *self.client_list.uuid(*client_id).unwrap(),
                        x: pos.x,
                        y: pos.y,
                        z: pos.z,
                        pitch: 0,
                        yaw: 0,
                    })
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        self.client_list.send_all(sender, player_packets);
    }

    #[allow(unused_variables)]
    async fn handle_client_status(&mut self, sender: ClientId, action_id: i32) {}

    #[allow(unused_variables)]
    async fn handle_chat_message(&mut self, sender: ClientId, message: &str) {
        if let Some(command) = message.strip_prefix('/') {
            let write_handle = self.client_list.create_write_handle(sender).unwrap();
            let executor = command_executor();
            let ctx = CommandContext::new(self, executor, CommandSender::Client(write_handle));

            match executor.dispatch(command, ctx) {
                Ok(_) => {}
                Err(e) => self.client_list.send_system_error(sender, &e).unwrap(),
            };
        } else {
            self.client_list.send_chat(sender, message);
        }
    }

    #[allow(unused_variables)]
    async fn handle_set_difficulty(&mut self, sender: ClientId, new_difficulty: i8) {}

    #[allow(unused_variables)]
    async fn handle_query_entity_nbt(
        &mut self,
        sender: ClientId,
        trasaction_id: i32,
        entity_id: i32,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_query_block_nbt(
        &mut self,
        sender: ClientId,
        trasaction_id: i32,
        location: &BlockPosition,
    ) {
    }

    #[allow(unused_variables)]
    async fn handle_teleport_confirm(&mut self, sender: ClientId, teleport_id: i32) {
        // Teleport id 1 *should* only be used for spawning players
        if teleport_id == 1 {
            // Ensure that we have spawned the player
            if let Some(player_world) = self.world_store.get_player_world_mut(sender) {
                let player_entity = *player_world.get_player_entity(sender).unwrap();
                let entities = player_world.get_entities_mut().await;
                let mut player_state = entities.get_mut::<PlayerState>(player_entity).unwrap();

                // Change the player's state to ready
                if let PlayerState::Spawning = *player_state {
                    *player_state = PlayerState::Ready;
                }
            }
        }
    }
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
                else if let Err(e) =
                    handle_packet(&mut conn, &mut async_handler, packet_len).await
                {
                    error!("Failed to handle packet: {}", e);
                    conn.write_handle.shutdown();
                    break;
                }
            }

            Err(e) => {
                error!("Error in connection handler: {}", e);
                conn.write_handle.shutdown();
                break;
            }
        }
    }

    conn.forward_internal_to_server(WrappedServerBoundPacket::ClientDisconnected { id: conn.id });
    debug!("Client disconnected");
}

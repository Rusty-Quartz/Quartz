use log::{debug, warn, error};

use serde::Deserialize;
use serde_json::json;

use openssl::rsa::{Rsa, Padding};
use openssl::pkey::Private;
use openssl::sha;

use std::sync::Arc;

use rand::{thread_rng, Rng};

use regex::Regex;

use lazy_static::lazy_static;

use hex::ToHex;

use crate::network::connection::{AsyncClientConnection, ConnectionState};
use crate::util::ioutil::ByteBuffer;
use crate::server::{self, QuartzServer};
use crate::data::Uuid;

pub const PROTOCOL_VERSION: i32 = 578;
pub const LEGACY_PING_PACKET_ID: i32 = 0xFE;

struct AsyncPacketHandler {
    key_pair: Arc<Rsa<Private>>,
    username: String,
    verify_token: Vec<u8>
}

impl AsyncPacketHandler {
    fn new(key_pair: Arc<Rsa<Private>>) -> Self {
        AsyncPacketHandler {
            key_pair,
            username: String::new(),
            verify_token: Vec::new()
        }
    }
}

impl AsyncPacketHandler {
//#AsyncPacketHandler
    fn handshake(&mut self, conn: &mut AsyncClientConnection, version: i32, next_state: i32) {
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

    fn ping(&mut self, conn: &mut AsyncClientConnection, payload: i64) {
        conn.send_packet(&ClientBoundPacket::Pong {payload});
    }

    fn login_start(&mut self, conn: &mut AsyncClientConnection, name: &str) {
        // Store username for later
        self.username = String::from(name);

        // Generate and store verify token
        let mut verify_token = [0_u8; 4];
        thread_rng().fill(&mut verify_token);
        self.verify_token = verify_token.to_vec();
        
        // Format public key to send to client
        let pub_key_der = self.key_pair.public_key_to_der().unwrap(); 

        conn.send_packet(&ClientBoundPacket::EncryptionRequest {
            server_id: String::from(""),
            pub_key_len: pub_key_der.len() as i32,
            pub_key: pub_key_der,
            verify_token_len: verify_token.len() as i32,
            verify_token: verify_token.to_vec()
        })
    }

    fn encryption_response(&mut self, conn: &mut AsyncClientConnection, shared_secret: &Vec<u8>, verify_token: &Vec<u8>) {

        // Decrypt and check verify token
        let mut decrypted_verify = vec![0; self.key_pair.size() as usize];
        self.key_pair.private_decrypt(verify_token, &mut decrypted_verify, Padding::PKCS1).unwrap();
        decrypted_verify = decrypted_verify[..self.verify_token.len()].to_vec();

        if self.verify_token != decrypted_verify {
            error!("verify for client {} didn't match, {:x?}, {:x?}", conn.id, self.verify_token, decrypted_verify);
            return conn.send_packet(&ClientBoundPacket::Disconnect {
                reason: String::from("Error verifying encryption")
            });
        }

        // Decrypt shared secret
        let mut decrypted_secret = vec![0; self.key_pair.size() as usize];
        self.key_pair.private_decrypt(shared_secret, &mut decrypted_secret, Padding::PKCS1).unwrap();
        decrypted_secret = decrypted_secret[..16].to_vec();
        
        // Generate server id hash
        let mut hasher = sha::Sha1::new();
        
        hasher.update(decrypted_secret.as_slice());
        hasher.update(&*self.key_pair.public_key_to_der().unwrap());
        
        let mut hash = hasher.finish();
        let hash_hex;
        
        // Big thanks to https://gist.github.com/RoccoDev/8fa130f1946f89702f799f89b8469bc9 for writing this minecraft hashing code
        lazy_static! {
            static ref LEADING_ZERO_REGEX: Regex = Regex::new(r#"^0+"#).unwrap();
        }

        let negative = (hash[0] & 0x80) == 0x80;
        
        if negative {
            let mut carry = true;
            for i in (0..hash.len()).rev() {
                hash[i] = !hash[i] & 0xff;
                if carry {
                    carry = hash[i] == 0xff;
                    hash[i] = hash[i] + 1;
                }
            }
            
            hash_hex = format!("-{}", LEADING_ZERO_REGEX.replace(&hash.encode_hex::<String>(), ""));
        }
        else {
            hash_hex = LEADING_ZERO_REGEX.replace(&hash.encode_hex::<String>(), "").to_string();
        }


        // use hash and username to generate link to mojang's servers
        // TODO: Implement prevent-proxy-connections by adding client ip to post req
        let url = format!("https://sessionserver.mojang.com/session/minecraft/hasJoined?username={}&serverId={}", &self.username, &hash_hex);

        // Structs used to allow serde to parse response json into struct
        #[derive(Deserialize)]
        #[allow(unused)]
        struct Properties {
            name: String,
            value: String,
            signature: String
        }

        #[derive(Deserialize)]
        #[allow(unused)]
        struct AuthResponse {
            id: String,
            name: String,
            properties: [Properties; 1]
        }

        // Make a put request
        let res: AuthResponse = reqwest::blocking::get(&url).unwrap().json().unwrap();

        // Initiate encryption
        conn.initiate_encryption(decrypted_secret.as_slice());

        // Currently disabled cause no need rn, will enable via config later
        // conn.send_packet(&ClientBoundPacket::SetCompression{threshhold: /* maximum size of uncompressed packet */})

        conn.send_packet(&ClientBoundPacket::LoginSuccess {
            uuid: format!("{}", Uuid::from_string(&res.id).unwrap()),
            username: self.username.clone()
        });
    }

    fn login_plugin_response(&mut self, _conn: &mut AsyncClientConnection, _message_id: i32, _successful: bool, _data: &Vec<u8>) {

    }
//#end
}

impl QuartzServer {
//#SyncPacketHandler
    fn login_success_server(&mut self, sender: usize, uuid: &str, username: &str) {
        
    }

    fn legacy_ping(&mut self, sender: usize) {
        // Load in all needed values from server object
        let protocol_version = u16::to_string(&(PROTOCOL_VERSION as u16));
        let version = server::VERSION;
        let motd = &self.config.motd;
        let player_count = self.client_list.online_count().to_string();
        let max_players = self.config.max_players.to_string();

        // Add String header
        let mut string_vec:Vec<u16> = vec![0x00A7, 0x0031, 0x0000];

        // Add all fields to vector
        string_vec.append(&mut protocol_version.chars().rev().collect::<String>().encode_utf16().collect::<Vec<u16>>());
        string_vec.push(0x0000);

        string_vec.append(&mut version.encode_utf16().collect::<Vec<u16>>());
        string_vec.push(0x0000);

        string_vec.append(&mut motd.as_plain_text().encode_utf16().collect::<Vec<u16>>());
        string_vec.push(0x0000);

        string_vec.append(&mut player_count.encode_utf16().collect::<Vec<u16>>());
        string_vec.push(0x0000);

        string_vec.append(&mut max_players.encode_utf16().collect::<Vec<u16>>());

        let mut buffer = ByteBuffer::new(3 + string_vec.len());

        // Write FF and length
        buffer.write_bytes(&[0xFF]);
        buffer.write_u16(string_vec.len() as u16);

        // Write String
        for bytes in string_vec {
            buffer.write_u16(bytes);
        }

        self.client_list.send_buffer(sender, &buffer);
    }

    fn status_request(&mut self, sender: usize) {
        let json_response = json!({
            "version": {
                "name": server::VERSION,
                "protocol": PROTOCOL_VERSION
            },
            "players": {
                "max": self.config.max_players,
                "online": self.client_list.online_count(),
                "sample": [] // Maybe implement this in the future
            },
            "description": self.config.motd
        });

        // TODO: implement favicon

        self.client_list.send_packet(sender, &ClientBoundPacket::StatusResponse {
            json_response: json_response.to_string()
        });
    }
//#end
}

pub enum ServerBoundPacket {
//#ServerBoundPacket
    LoginSuccessServer {
        uuid: String, 
        username: String
    },
    LegacyPing,
    StatusRequest
//#end
}

pub struct WrappedServerPacket {
    pub sender: usize,
    pub packet: ServerBoundPacket
}

impl WrappedServerPacket {
    #[inline]
    pub fn new(sender: usize, packet: ServerBoundPacket) -> Self {
        WrappedServerPacket {
            sender,
            packet
        }
    }
}

pub enum ClientBoundPacket {
//#ClientBoundPacket
    StatusResponse {
        json_response: String
    },
    Pong {
        payload: i64
    },
    Disconnect {
        reason: String
    },
    EncryptionRequest {
        server_id: String, 
        pub_key_len: i32, 
        pub_key: Vec<u8>, 
        verify_token_len: i32, 
        verify_token: Vec<u8>
    },
    LoginSuccess {
        uuid: String, 
        username: String
    },
    SetCompression {
        threshold: i32
    },
    LoginPluginRequest {
        message_id: i32, 
        channel: String, 
        data: Vec<u8>
    }
//#end
}

pub fn dispatch_sync_packet(wrapped_packet: &WrappedServerPacket, handler: &mut QuartzServer) {
//#dispatch_sync_packet
    match &wrapped_packet.packet {
        ServerBoundPacket::LoginSuccessServer {uuid, username} => handler.login_success_server(wrapped_packet.sender, uuid, username),
        ServerBoundPacket::LegacyPing => handler.legacy_ping(wrapped_packet.sender),
        ServerBoundPacket::StatusRequest => handler.status_request(wrapped_packet.sender)
    }
//#end
}

pub fn serialize(packet: &ClientBoundPacket, buffer: &mut ByteBuffer) {
//#serialize
    match packet {
        ClientBoundPacket::StatusResponse {json_response} => {
            buffer.write_varint(0x00);
            buffer.write_string(json_response);
        },
        ClientBoundPacket::Pong {payload} => {
            buffer.write_varint(0x01);
            buffer.write_i64(*payload);
        },
        ClientBoundPacket::Disconnect {reason} => {
            buffer.write_varint(0x00);
            buffer.write_string(reason);
        },
        ClientBoundPacket::EncryptionRequest {server_id, pub_key_len, pub_key, verify_token_len, verify_token} => {
            buffer.write_varint(0x01);
            buffer.write_string(server_id);
            buffer.write_varint(*pub_key_len);
            buffer.write_byte_array(pub_key);
            buffer.write_varint(*verify_token_len);
            buffer.write_byte_array(verify_token);
        },
        ClientBoundPacket::LoginSuccess {uuid, username} => {
            buffer.write_varint(0x02);
            buffer.write_string(uuid);
            buffer.write_string(username);
        },
        ClientBoundPacket::SetCompression {threshold} => {
            buffer.write_varint(0x03);
            buffer.write_varint(*threshold);
        },
        ClientBoundPacket::LoginPluginRequest {message_id, channel, data} => {
            buffer.write_varint(0x04);
            buffer.write_varint(*message_id);
            buffer.write_string(channel);
            buffer.write_byte_array(data);
        }
    }
//#end
}

macro_rules! invalid_packet {
    ($id:expr, $len:expr) => {
        warn!("Invalid packet received. ID: {}, Len: {}", $id, $len);
    };
}

fn handle_packet(conn: &mut AsyncClientConnection, async_handler: &mut AsyncPacketHandler, packet_len: usize) {
    let buffer = &mut conn.read_buffer;
    let id;
    if conn.connection_state == ConnectionState::Handshake && buffer.peek() == LEGACY_PING_PACKET_ID as u8 {
        id = LEGACY_PING_PACKET_ID;
    } else {
        id = buffer.read_varint();
    }

//#handle_packet
    match conn.connection_state {
        ConnectionState::Handshake => {
            match id {
                0x00 => {
                    let version = buffer.read_varint();
                    buffer.read_string(); // server_address
                    buffer.read_u16(); // server_port
                    let next_state = buffer.read_varint();
                    async_handler.handshake(conn, version, next_state);
                },
                LEGACY_PING_PACKET_ID => {
                    buffer.read_byte_array(buffer.remaining()); // data
                    conn.forward_to_server(ServerBoundPacket::LegacyPing);
                },
                _ => invalid_packet!(id, buffer.len())
            }
        },
        ConnectionState::Status => {
            match id {
                0x00 => {
                    conn.forward_to_server(ServerBoundPacket::StatusRequest);
                },
                0x01 => {
                    let payload = buffer.read_i64();
                    async_handler.ping(conn, payload);
                },
                _ => invalid_packet!(id, buffer.len())
            }
        },
        ConnectionState::Login => {
            match id {
                0x00 => {
                    let name = buffer.read_string();
                    async_handler.login_start(conn, &name);
                },
                0x01 => {
                    let shared_secret_len = buffer.read_varint();
                    let shared_secret = buffer.read_byte_array(shared_secret_len as usize);
                    let verify_token_len = buffer.read_varint();
                    let verify_token = buffer.read_byte_array(verify_token_len as usize);
                    async_handler.encryption_response(conn, &shared_secret, &verify_token);
                },
                0x02 => {
                    let message_id = buffer.read_varint();
                    let successful = buffer.read_bool();
                    let data = buffer.read_byte_array(packet_len - buffer.cursor());
                    async_handler.login_plugin_response(conn, message_id, successful, &data);
                },
                _ => invalid_packet!(id, buffer.len())
            }
        },
        _ => {}
    }
//#end
}

pub fn handle_async_connection(mut conn: AsyncClientConnection, private_key: Arc<Rsa<Private>>) {
    let mut async_handler = AsyncPacketHandler::new(private_key);

    while conn.connection_state != ConnectionState::Disconnected {
        match conn.read_packet() {
            Ok(packet_len) => {
                // Client disconnected
                if packet_len == 0 {
                    break;
                }
                // Handle the packet
                else {
                    handle_packet(&mut conn, &mut async_handler, packet_len);
                }
            },
            Err(e) => {
                // TODO: handle properly
                error!("Error in connection handler: {}", e);
                break;
            }
        }
    }

    debug!("Client disconnected");
}
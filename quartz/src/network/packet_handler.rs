use std::str::FromStr;
use std::sync::{
    Arc,
    mpsc::Sender
};
use log::{debug, warn, error};
use serde::Deserialize;
use serde_json::json;
use openssl::rsa::{Rsa, Padding};
use openssl::pkey::Private;
use openssl::sha;
use rand::{thread_rng, Rng};
use regex::Regex;
use lazy_static::lazy_static;
use hex::ToHex;
use util::Uuid;
use crate::Registry;
use crate::network::{AsyncClientConnection, ConnectionState, PacketBuffer};
use crate::server::{self, QuartzServer};
use crate::command::CommandSender;

/// The numeric protocol version the server uses.
pub const PROTOCOL_VERSION: i32 = 736;
/// The ID for the legacy ping packet.
pub const LEGACY_PING_PACKET_ID: i32 = 0xFE;

include!(concat!(env!("OUT_DIR"), "/packet_output.rs"));

/// A wraper for a server-bound packet which includes the sender ID.
pub struct WrappedServerBoundPacket {
    /// The ID of the packet sender.
    pub sender: usize,
    /// The packet that was sent.
    pub packet: ServerBoundPacket
}

impl WrappedServerBoundPacket {
    /// Creates a new wrapper with the given parameters.
    #[inline]
    pub fn new(sender: usize, packet: ServerBoundPacket) -> Self {
        WrappedServerBoundPacket {
            sender,
            packet
        }
    }
}

/// A wraper for client-bound packets used internally for sending packets to the connection thread.
pub enum WrappedClientBoundPacket {
    /// A wrapped packet.
    Packet(ClientBoundPacket),
    /// A raw byte-buffer.
    Buffer(PacketBuffer),
    /// Specifies that the connection should be forcefully terminated.
    Disconnect
}

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
            pub_key_len: pub_key_der.len() as i32,
            pub_key: pub_key_der,
            verify_token_len: verify_token.len() as i32,
            verify_token: verify_token.to_vec()
        })
    }

    fn encryption_response(&mut self, conn: &mut AsyncClientConnection, shared_secret: &Vec<u8>, verify_token: &Vec<u8>) {
        // Decrypt and check verify token
        let mut decrypted_verify = vec![0; self.key_pair.size() as usize];
        if let Err(e) = self.key_pair.private_decrypt(verify_token, &mut decrypted_verify, Padding::PKCS1) {
            error!("Failed to decrypt verify token: {}", e);
            conn.shutdown();
            return;
        }
        decrypted_verify = decrypted_verify[..self.verify_token.len()].to_vec();

        if self.verify_token != decrypted_verify {
            error!("verify for client {} didn't match, {:x?}, {:x?}", conn.id, self.verify_token, decrypted_verify);
            return conn.send_packet(&ClientBoundPacket::Disconnect {
                reason: "Error verifying encryption".to_owned()
            });
        }

        // Decrypt shared secret
        let mut decrypted_secret = vec![0; self.key_pair.size() as usize];
        if let Err(e) = self.key_pair.private_decrypt(shared_secret, &mut decrypted_secret, Padding::PKCS1) {
            error!("Failed to decrypt secret key: {}", e);
            conn.shutdown();
            return;
        }
        decrypted_secret = decrypted_secret[..16].to_vec();

        // Initiate encryption
        if let Err(e) = conn.initiate_encryption(decrypted_secret.as_slice()) {
            error!("Failed to initialize encryption for client connetion: {}", e);
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

        // Currently disabled cause no need rn, will enable via config later
        // conn.send_packet(&ClientBoundPacket::SetCompression{threshhold: /* maximum size of uncompressed packet */})

        // Make a get request
        let mojang_req = ureq::get(&url).call();
        if mojang_req.ok() {
            match mojang_req.into_json_deserialize::<AuthResponse>() {
                Ok(json) => match Uuid::from_str(&json.id) {
                    Ok(uuid) => conn.send_packet(&ClientBoundPacket::LoginSuccess {
                        uuid,
                        username: self.username.clone()
                    }),
                    Err(e) => error!("Malformed UUID in auth resonse: {}", e) 
                },
                Err(e) => error!("Failed to upack JSON from session server response: {}", e)
            }
        }
        else {
            error!("Failed to make session server request")
        }    
    }

    fn login_plugin_response(&mut self, _conn: &mut AsyncClientConnection, _message_id: i32, _successful: bool, _data: &Vec<u8>) {
        // TODO: Implement login_plugin_response
    }
}

impl<R: Registry> QuartzServer<R> {
    fn login_success_server(&mut self, _sender: usize, _uuid: &Uuid, _username: &str) {
        // TODO: Implement login_success_server
    }

    fn handle_console_command(&mut self, command: &str) {
        let command_executor = self.command_executor.clone();
        match command_executor.try_borrow() {
            Ok(executor) => {
                let sender = CommandSender::Console(self.console_interface.clone());
                executor.dispatch(command, self, sender);
            },
            Err(_) => error!("Internal error: could not borrow command_executor as mutable while executing a command.")
        };
    }

    fn handle_console_completion(&mut self, command: &str, response: &Sender<Vec<String>>) {
        let command_executor = self.command_executor.clone();
        match command_executor.try_borrow() {
            Ok(executor) => {
                let sender = CommandSender::Console(self.console_interface.clone());
                let suggestions = executor.get_suggestions(command, self, sender);
                // Error handling not useful here
                drop(response.send(suggestions));
            },
            Err(_) => error!("Internal error: could not borrow command_executor as mutable while generating completion suggestions.")
        };
    }

    fn legacy_ping(&mut self, sender: usize) {
        // Load in all needed values from server object
        let protocol_version = u16::to_string(&(PROTOCOL_VERSION as u16));
        let version = server::VERSION;
        let motd = &self.config.motd;
        let player_count = self.client_list.online_count().to_string();
        let max_players = self.config.max_players.to_string();

        // Add String header
        let mut string_vec: Vec<u16> = vec![0x00A7, 0x0031, 0x0000];

        // Add all fields to vector
        string_vec.extend(protocol_version.chars().rev().collect::<String>().encode_utf16());
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
        buffer.write_u16(string_vec.len() as u16);

        // Write String
        for bytes in string_vec {
            buffer.write_u16(bytes);
        }

        self.client_list.send_buffer(sender, buffer);
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
                "sample": [] // TODO: Decide whether or not to implement "sample" in status req
            },
            "description": self.config.motd
        });

        // TODO: implement favicon

        self.client_list.send_packet(sender, ClientBoundPacket::StatusResponse {
            json_response: json_response.to_string()
        });
    }
}

/// Handles the given asynchronos connecting using blocking I/O opperations.
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
                error!("Error in connection handler: {}", e);
                conn.shutdown();
                break;
            }
        }
    }

    conn.forward_to_server(ServerBoundPacket::ClientDisconnected {
        id: conn.id
    });
    debug!("Client disconnected");
}
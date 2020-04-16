use crate::network::connection::{AsyncClientConnection, ConnectionState};
use crate::util::ioutil::ByteBuffer;
use crate::server::QuartzServer;

const PROTOCOL_VERSION: i32 = 578;

struct AsyncPacketHandler {

}

impl AsyncPacketHandler {
	pub fn new() -> AsyncPacketHandler {
		AsyncPacketHandler {}
	}

//#AsyncPacketHandler
	async fn handshake(&mut self, conn: &mut AsyncClientConnection, version: i32, server_address: String, server_port: u16, next_state: i32) {
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

	async fn ping(&mut self, conn: &mut AsyncClientConnection, payload: i64) {
		conn.send_packet(ClientBoundPacket::Pong {payload}).await;
	}

	async fn login_start(&mut self, conn: &mut AsyncClientConnection, name: String) {

	}

	async fn encryption_response(&mut self, conn: &mut AsyncClientConnection, shared_secret_len: i32, shared_secret: Vec<u8>, verify_token_len: i32, verify_token: Vec<u8>) {

	}

	async fn login_plugin_response(&mut self, conn: &mut AsyncClientConnection, message_id: i32, successful: bool, data: Vec<u8>) {

	}
//#end
}

impl QuartzServer {
//#SyncPacketHandler
	async fn legacy_ping(&mut self, payload: u8) {

	}

	async fn request(&mut self) {

	}

	async fn login_success_server(&mut self, uuid: String, username: String) {

	}
//#end
}

pub enum ServerBoundPacket {
//#ServerBoundPacket
	LegacyPing {
		payload: u8
	},
	Request,
	LoginSuccessServer {
		uuid: String, 
		username: String
	}
//#end
}

pub enum ClientBoundPacket {
//#ClientBoundPacket
	Response {
		json_length: i32, 
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

pub async fn dispatch_sync_packet(packet: ServerBoundPacket, handler: &mut QuartzServer) {
//#dispatch_sync_packet
	match packet {
		ServerBoundPacket::LegacyPing{payload} => handler.legacy_ping(payload).await,
		ServerBoundPacket::Request => handler.request().await,
		ServerBoundPacket::LoginSuccessServer{uuid, username} => handler.login_success_server(uuid, username).await,
		_ => {}
	}
//#end
}

pub fn serialize(packet: ClientBoundPacket, buffer: &mut ByteBuffer) {
//#serialize
	match packet {
		ClientBoundPacket::Response{json_length, json_response} => {
			buffer.write_varint(0);
			buffer.write_varint(json_length);
			buffer.write_string(&json_response);
		},
		ClientBoundPacket::Pong{payload} => {
			buffer.write_varint(1);
			buffer.write_i64(payload);
		},
		ClientBoundPacket::Disconnect{reason} => {
			buffer.write_varint(0);
			buffer.write_string(&reason);
		},
		ClientBoundPacket::EncryptionRequest{server_id, pub_key_len, pub_key, verify_token_len, verify_token} => {
			buffer.write_varint(1);
			buffer.write_string(&server_id);
			buffer.write_varint(pub_key_len);
			buffer.write_byte_array(&pub_key);
			buffer.write_varint(verify_token_len);
			buffer.write_byte_array(&verify_token);
		},
		ClientBoundPacket::LoginSuccess{uuid, username} => {
			buffer.write_varint(2);
			buffer.write_string(&uuid);
			buffer.write_string(&username);
		},
		ClientBoundPacket::SetCompression{threshold} => {
			buffer.write_varint(3);
			buffer.write_varint(threshold);
		},
		ClientBoundPacket::LoginPluginRequest{message_id, channel, data} => {
			buffer.write_varint(4);
			buffer.write_varint(message_id);
			buffer.write_string(&channel);
			buffer.write_byte_array(&data);
		}
	}
//#end
}

async fn handle_packet(conn: &mut AsyncClientConnection, async_handler: &mut AsyncPacketHandler) {
    let mut buffer = &mut conn.packet_buffer;
    let id = buffer.read_varint();

//#handle_packet
	match conn.connection_state {
		ConnectionState::Handshake => {
			match id {
				0x00 => {
					let version = buffer.read_varint();
					let server_address = buffer.read_string();
					let server_port = buffer.read_u16();
					let next_state = buffer.read_varint();
					async_handler.handshake(conn, version, server_address, server_port, next_state).await;
				},
				0xFE => {
					let payload = buffer.read_u8();
					conn.forward_to_server(ServerBoundPacket::LegacyPing {payload});
				},
				_ => invalid_packet(id, buffer.len())
			}
		},
		ConnectionState::Status => {
			match id {
				0x00 => {
					conn.forward_to_server(ServerBoundPacket::Request);
				},
				0x01 => {
					let payload = buffer.read_i64();
					async_handler.ping(conn, payload).await;
				}
				_ => invalid_packet(id, buffer.len())
			}
		},
		ConnectionState::Login => {
			match id {
				0x00 => {
					let name = buffer.read_string();
					async_handler.login_start(conn, name).await;
				},
				0x01 => {
					let shared_secret_len = buffer.read_varint();
					let shared_secret = buffer.read_byte_array(shared_secret_len as usize);
					let verify_token_len = buffer.read_varint();
					let verify_token = buffer.read_byte_array(verify_token_len as usize);
					async_handler.encryption_response(conn, shared_secret_len, shared_secret, verify_token_len, verify_token).await;
				},
				0x02 => {
					let message_id = buffer.read_varint();
					let successful = buffer.read_bool();
					let data = buffer.read_byte_array(buffer.remaining());
					async_handler.login_plugin_response(conn, message_id, successful, data).await;
				},
				0xFF => {
					let uuid = buffer.read_string();
					let username = buffer.read_string();
					conn.forward_to_server(ServerBoundPacket::LoginSuccessServer {uuid,username});
				},
				_ => invalid_packet(id, buffer.len())
			}
		},
		_ => {}
	}
//#end
}

#[inline(always)]
fn invalid_packet(id: i32, len: usize) {
	println!("Invalid packet received. ID: {}, Len: {}", id, len);
}

pub async fn handle_async_connection(mut conn: AsyncClientConnection) {
    let mut async_handler = AsyncPacketHandler::new();

    while conn.connection_state != ConnectionState::Disconnected {
        match conn.read_packet().await {
            Ok(_) => handle_packet(&mut conn, &mut async_handler).await,
            Err(e) => {
                // TODO: handle properly
                println!("Error in connection handler: {}", e);
                return;
            }
        }
    }

    println!("Client disconnected.");
}
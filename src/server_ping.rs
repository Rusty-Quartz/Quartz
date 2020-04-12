use crate::tcp_socket_helper::read_packet;
use crate::mc_datatype_helpers::write_varint;
use crate::server::{RedstoneServer, Player};

use std::net::TcpStream;
use std::io::Write;
use serde::{Serialize};

#[derive(Serialize)]
pub struct ServerStatusResponse {
	version: McVersion,
	players: ServerPlayersObject,
	description: ChatObject
}

#[derive(Serialize)]
struct McVersion {
	name: String,
	protocol: i32
}

#[derive(Serialize)]
struct ServerPlayersObject {
	max: u32,
	online: i32,
	sample: Vec<Player>
}

#[derive(Serialize)]
struct ChatObject {
	text: String
}


pub fn load_server_status(server: &RedstoneServer) -> ServerStatusResponse {
	ServerStatusResponse {
		version: McVersion {
			name: "Redstone 1.15.2".to_owned(),
			protocol: 578
		},
		players: ServerPlayersObject {
			max: server.config.max_players,
			online: server.players.len() as i32,
			sample: Vec::new()
		},
		description: ChatObject {
			text: server.config.motd.to_owned()
		}
	}
}

pub fn send_server_ping(stream: &mut TcpStream, server: &RedstoneServer) {
	let mut buffer = [0_u8; 4096];

	let res = load_server_status(server);

	read_packet(stream, &mut buffer);
	//read_packet(stream, &mut buffer);

	let res_json = serde_json::to_string(&res).unwrap();

	println!("{}", res_json);

	let len: i32 = res_json.len() as i32;

	let mut res_array = [0_u8; 1024];

	let mut json_len_bytes = 0;
	let mut len_copy = len;
	while len_copy != 0 {
		len_copy >>= 7;
		json_len_bytes += 1;
	}

	println!("Packet Len: {}", 2 + len);
	let mut off = write_varint(1 + json_len_bytes + len, &mut res_array, 0);
	println!("Packet len off: {}", off);
	off += write_varint(0, &mut res_array, off);
	off += write_varint(len, &mut res_array, off);
	println!("json len: {}", len);


	for byte in res_json.as_bytes() {
		res_array[off] = *byte;
		off += 1;
	}

	println!("Writing Data");
	for i in 0..(3 + len) {
		let x = res_array[i as usize];
		print!("{:02x?},", x);
	}

	stream.write(&res_array).unwrap();
	stream.flush().unwrap();

	read_packet(stream, &mut buffer);
	stream.write(&buffer).unwrap();
	stream.flush().unwrap();
}
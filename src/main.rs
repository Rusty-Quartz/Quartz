mod server_ping;
mod server;
mod config;
mod tcp_socket_helper;
mod mc_datatype_helpers;

use std::net::{TcpListener};
use std::io::Result;

use config::load_config;
use server::QuartzServer;
use network::{connection::ClientConnection, packet_handler::handle_connection};

mod util {
	pub mod ioutil;
}

mod network {
	pub mod connection;
	pub mod packet_handler;
}

fn main() -> Result<()> {
	let config = load_config(String::from("./server.properties"));

	let server = QuartzServer {
		players: Vec::new(),
		config: config,
		debug: true
	};

	let listener = TcpListener::bind(format!("127.0.0.1:{}", server.config.port))?;
	
	for stream in listener.incoming() {
		println!("client connecting!");
		handle_connection(ClientConnection::new(stream?));
	}

	Ok(())
}
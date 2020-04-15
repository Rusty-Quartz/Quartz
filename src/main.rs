mod server_ping;
mod server;
mod config;
mod tcp_socket_helper;
mod mc_datatype_helpers;

use std::io::Result;

use config::load_config;
use server::QuartzServer;
use network::{connection::AsyncClientConnection, packet_handler::{handle_async_connection, Packet}};
use tokio::sync::mpsc;
use std::net::TcpListener;

mod util {
	pub mod ioutil;
}

mod network {
	pub mod connection;
	pub mod packet_handler;
}

#[tokio::main]
async fn main() -> Result<()> {
	let config = load_config(String::from("./config.json"));
	let (sync_packet_sender, sync_packet_receiver) = mpsc::unbounded_channel::<Packet>();

	let server = QuartzServer {
		players: Vec::new(),
		config: config,
		debug: true
	};

	let listener = TcpListener::bind(format!("127.0.0.1:{}", server.config.port))?;
	
	loop {
		let (stream, addr) = listener.accept()?;

		println!("Client connected.");
		let conn = AsyncClientConnection::new(stream, sync_packet_sender.clone());
		let write_handle = conn.create_write_handle();

		tokio::spawn(async move {
			handle_async_connection(conn).await;
		});
	}
}
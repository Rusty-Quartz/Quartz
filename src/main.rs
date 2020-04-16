mod config;

use std::io::Result;

use config::load_config;
use network::{connection::AsyncClientConnection, packet_handler::{handle_async_connection, ServerBoundPacket}};
use tokio::sync::mpsc;
use std::net::TcpListener;

mod util {
	pub mod ioutil;
}

mod network {
	pub mod connection;
	pub mod packet_handler;
}

mod data {
	pub use self::uuid::Uuid;

	pub mod uuid;
}

mod server;

#[tokio::main]
async fn main() -> Result<()> {
	let config = load_config(String::from("./config.json"));
	let (sync_packet_sender, sync_packet_receiver) = mpsc::unbounded_channel::<ServerBoundPacket>();

	let server = server::QuartzServer {
		config,
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
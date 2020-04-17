mod config;

use std::io::Result;

use config::load_config;
use network::{connection::AsyncClientConnection, packet_handler::{handle_async_connection, WrappedServerPacket}};
use tokio::sync::mpsc;
use std::net::TcpListener;

pub mod data {
	mod uuid;

	pub use self::uuid::Uuid;
}

pub mod nbt {
	mod tag;
	pub mod read;
	pub mod write;

	pub use self::tag::NbtTag;
	pub use self::tag::NbtCompound;
	pub use self::tag::NbtList;
}

pub mod network {
	pub mod connection;
	pub mod packet_handler;
}

pub mod util {
	pub mod ioutil;
}

mod server;

#[tokio::main]
async fn main() -> Result<()> {
	let config = load_config(String::from("./config.json"));
	let (sync_packet_sender, sync_packet_receiver) = mpsc::unbounded_channel::<WrappedServerPacket>();

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
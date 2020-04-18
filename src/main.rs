mod config;

use std::error::Error;
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;
use std::io;
use std::time::Duration;

use futures::channel::mpsc;

use linefeed::Interface;

use log::*;

use config::load_config;
use network::{
	connection::AsyncClientConnection,
	packet_handler::{
		handle_async_connection,
		ServerBoundPacket,
		WrappedServerPacket
	}
};

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

mod logging;
mod server;

fn main() -> Result<(), Box<dyn Error>> {
	let console_interface = Arc::new(Interface::new("quartz-server")?);
	console_interface.set_prompt("> ")?;

	logging::init_logger(console_interface.clone())?;

	let config = load_config(String::from("./config.json"));
	let (sync_packet_sender, sync_packet_receiver) = mpsc::unbounded::<WrappedServerPacket>();

	let listener = TcpListener::bind(format!("127.0.0.1:{}", config.port))?;
	let server = server::init_server(config, sync_packet_receiver);

	let mut next_connection_id: usize = 0;
	thread::spawn(move || {
		loop {
			match listener.accept() {
				// Successful connection
				Ok((socket, _addr)) => {
					info!("Client connected.");
					let packet_sender = sync_packet_sender.clone();
					let mut conn = AsyncClientConnection::new(next_connection_id, socket, packet_sender);
					next_connection_id += 1;

					let write_handle = conn.create_write_handle();
					conn.forward_to_server(ServerBoundPacket::ConnectionEstablished {write_handle});

					thread::spawn(move || {
						handle_async_connection(conn);
					});
				},

				// Somewhat patchy shutdown hook
				Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => return,

				// Actual error
				Err(e) => error!("Failed to accept TCP socket: {}", e)
			};
		}
	});

	while server.running {
		// Do nothing for now
		thread::sleep(Duration::from_millis(50));
	}

	logging::cleanup();

	Ok(())
}


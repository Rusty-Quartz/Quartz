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
        WrappedServerPacket,
        ServerBoundPacket
    }
};
use server::QuartzServer;

pub mod chat {
    pub mod component;
}

pub mod data {
    mod uuid;

    pub use self::uuid::Uuid;
}

pub mod nbt {
    mod tag;
    pub mod read;
    pub mod write;
    pub mod snbt;

    pub use self::tag::NbtTag;
    pub use self::tag::NbtCompound;
    pub use self::tag::NbtList;
}

pub mod network {
    pub mod connection;
    pub mod packet_handler;
}

pub mod util {
    pub mod idlist;
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
    listener.set_nonblocking(true).expect("Failed to create non-blocking TCP listener");

    let mut server = QuartzServer::new(config, sync_packet_receiver);
    server.init();

    let server_handle = thread::spawn(move || {
        let mut next_connection_id: usize = 0;

        info!("Started TCP Server Thread");

        loop {
            match listener.accept() {
                // Successful connection
                Ok((socket, _addr)) => {
                    // Don't bother handling the connection if the server is shutting down
                    if !server::is_running() {
                        return;
                    }

                    debug!("Client connected");
                    let packet_sender = sync_packet_sender.clone();
                    let mut conn = AsyncClientConnection::new(next_connection_id, socket, packet_sender);
                    next_connection_id += 1;

                    conn.forward_to_server(ServerBoundPacket::AddClient {
                        connection: conn.create_write_handle()
                    });

                    thread::spawn(move || {
                        handle_async_connection(conn);
                    });
                },

                // Wait before checking for a new connection and exit if the server is no longer running
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if server::is_running() {
                        thread::sleep(Duration::from_millis(100));
                    } else {
                        return;
                    }
                },

                // Actual error
                Err(e) => error!("Failed to accept TCP socket: {}", e)
            };
        }
    });

    server.add_join_handle("TCP Server Thread", server_handle);

    server.run();

    drop(server);
    logging::cleanup();

    Ok(())
}


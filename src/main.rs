use std::error::Error;
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;
use std::io;
use std::time::Duration;

use futures::channel::mpsc;

use linefeed::Interface;

use log::*;

use config::*;
use network::{
    connection::AsyncClientConnection,
    packet_handler::{
        handle_async_connection,
        WrappedServerPacket
    }
};
use server::QuartzServer;

use openssl::rsa::Rsa;

pub mod block {
    pub mod block;
    mod init;
    pub mod state;

    pub use self::block::Block;
    pub use self::init::init_blocks;
    pub use self::state::{
        BlockState,
        StateData
    };
}

pub mod chat {
    pub mod component;
    #[macro_use]
    pub mod cfmt;
}

pub mod command {
    pub mod arg;
    pub mod executor;
    mod init;
    mod sender;

    pub use self::sender::CommandSender;
    pub use self::init::init_commands;
}

pub mod data {
    mod chunk;
    mod registry;
    mod uln;
    mod uuid;

    pub use self::chunk::Chunk;
    pub use self::registry::{
        Registry,
        StateID
    };
    pub use self::uln::UnlocalizedName;
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

mod config;
#[macro_use]
mod logging;
mod server;

fn main() -> Result<(), Box<dyn Error>> {
    let console_interface = Arc::new(Interface::new("quartz-server")?);
    console_interface.set_prompt("> ")?;

    logging::init_logger(console_interface.clone())?;

    let config: Config;
    match load_config(String::from("./config.json")) {
        Ok(cfg) => config = cfg,
        Err(e) => {
            error!("Failed to load config: {}", e);
            return Ok(())
        }
    }

    let (sync_packet_sender, sync_packet_receiver) = mpsc::unbounded::<WrappedServerPacket>();

    let listener = TcpListener::bind(format!("{}:{}", config.server_ip, config.port))?;
    listener.set_nonblocking(true).expect("Failed to create non-blocking TCP listener");

    let mut server = QuartzServer::new(config, sync_packet_receiver, console_interface);
    let client_list = server.client_list.clone();
    server.init(sync_packet_sender.clone());

    let server_handle = thread::spawn(move || {
        let mut next_connection_id: usize = 0;

        info!("Started TCP Server Thread");

        // If this fails a panic is justified
        let key_pair = Arc::new(Rsa::generate(1024).unwrap());

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
                    let conn = AsyncClientConnection::new(next_connection_id, socket, packet_sender);
                    next_connection_id += 1;

                    let key_pair_clone = key_pair.clone();

                    client_list.add_client(conn.id, conn.create_write_handle());

                    let client_list_clone = client_list.clone();
                    thread::spawn(move || {
                        let client_id = conn.id;
                        handle_async_connection(conn, key_pair_clone);
                        client_list_clone.remove_client(client_id);
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


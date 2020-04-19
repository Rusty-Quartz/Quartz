use futures::channel::mpsc::UnboundedReceiver;
use std::thread::{self, JoinHandle};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::borrow::Cow;
use std::time::Duration;
use crate::config::Config;
use crate::network::packet_handler::{WrappedServerPacket, ClientBoundPacket, dispatch_sync_packet, PROTOCOL_VERSION};
use crate::network::connection::WriteHandle;

use serde::Serialize;

use log::{info, warn, error};

static mut SERVER_INSTANCE: Option<QuartzServer> = None;
static mut SERVER_STATE: AtomicU8 = AtomicU8::new(0);

// No server object available, no processes directly tied to the server have been started
const UNITIALIZED: u8 = 0;

// The server is being constructed an initialized and should not be publically accessible
const INITIALIZING: u8 = 1;

// The server was constructed successfully and can safely be accessed
const INITIALIZED: u8 = 2;

// The server has been dropped or is in the process of doing so
const SHUTDOWN: u8 = 3;

pub fn init_server(
    config: Config,
    sync_packet_receiver: UnboundedReceiver<WrappedServerPacket>
) -> &'static mut QuartzServer<'static> {
    unsafe {
        match SERVER_STATE.compare_and_swap(UNITIALIZED, INITIALIZING, Ordering::SeqCst) {
            UNITIALIZED => {
                // Initialize the server
                let mut server = QuartzServer {
                    config,
                    client_list: Cow::Owned(HashMap::new()),
                    sync_packet_receiver,
                    join_handles: HashMap::new(),
                    version: "1.15.2"
                };
                server.init();
                SERVER_INSTANCE = Some(server);

                // The server object can now be safely accessed
                SERVER_STATE.store(INITIALIZED, Ordering::SeqCst);
    
                // Return a mutable reference to the server object
                SERVER_INSTANCE.as_mut().unwrap()
            },
            INITIALIZING | INITIALIZED => panic!("Attempted to initialize server more than once."),
            SHUTDOWN => panic!("Attempted to initialize server after shutdown."),
            _ => unreachable!("Invalid server state.")
        }
    }
}

pub fn add_client(id: usize, connection: WriteHandle) {
    unsafe {
        match SERVER_STATE.load(Ordering::Relaxed) {
            INITIALIZED => {
                SERVER_INSTANCE.as_mut().unwrap().add_client(id, connection);
            }
            UNITIALIZED | INITIALIZING => panic!("Attempted to add client before server was initialized."),
            SHUTDOWN => warn!("Client attempted to connect during server shutdown sequence."),
            _ => unreachable!("Invalid server state.")
        }
    }
}

pub fn remove_client(id: usize) {
    unsafe {
        match SERVER_STATE.load(Ordering::Relaxed) {
            INITIALIZED => {
                SERVER_INSTANCE.as_mut().unwrap().remove_client(id);
            }
            UNITIALIZED | INITIALIZING => panic!("Attempted to add client before server was initialized."),
            SHUTDOWN => warn!("Client attempted to connect during server shutdown sequence."),
            _ => unreachable!("Invalid server state.")
        }
    }
}

#[inline]
pub fn is_running() -> bool {
    unsafe { SERVER_STATE.load(Ordering::Relaxed) == INITIALIZED }
}

macro_rules! shutdown_internal {
    () => {
        {
            info!("Shutting down server");
            // Drops the server instance
            SERVER_INSTANCE = None;
        }
    };
}

pub fn shutdown_if_initialized() {
    unsafe {
        if SERVER_STATE.compare_and_swap(INITIALIZED, SHUTDOWN, Ordering::SeqCst) == INITIALIZED {
            shutdown_internal!();
        }
    }
}

pub fn shutdown() {
    unsafe {
        match SERVER_STATE.compare_and_swap(INITIALIZED, SHUTDOWN, Ordering::SeqCst) {
            INITIALIZED => shutdown_internal!(),
            UNITIALIZED | INITIALIZING => panic!("Attempted to shutdown server before it was initialized."),
            SHUTDOWN => panic!("Attempted to shutdown server more than once."),
            _ => unreachable!("Invalid server state.")
        }
    }
}

pub struct QuartzServer<'a> {
    pub config: Config,
    client_list: Cow<'a, HashMap<usize, Client>>,
    sync_packet_receiver: UnboundedReceiver<WrappedServerPacket>,
    join_handles: HashMap<String, JoinHandle<()>>,
    pub version: &'static str
}

impl<'a> QuartzServer<'a> {
    fn init(&mut self) {
        // In case it's needed later
    }

    pub fn add_join_handle(&mut self, thread_name: &str, handle: JoinHandle<()>) {
        self.join_handles.insert(thread_name.to_owned(), handle);
    }

    pub fn add_client(&mut self, client_id: usize, connection: WriteHandle) {
        self.client_list.to_mut().insert(client_id, Client::new(connection));
    }

    pub fn remove_client(&mut self, client_id: usize) {
        self.client_list.to_mut().remove(&client_id);
    }

    pub fn send_packet(&self, client_id: usize, packet: ClientBoundPacket) {
        match self.client_list.get(&client_id) {
            Some(client) => client.send_packet(packet),
            None => error!("Could not find client with ID {}, failed to send packet", client_id)
        }
    }

    pub fn run(&mut self) {
        loop {
            loop {
                match self.sync_packet_receiver.try_next() {
                    Ok(packet_wrapper) => {
                        match packet_wrapper {
                            Some(packet) => {
                                dispatch_sync_packet(&packet, self);
                            },
                            None => break
                        }
                    },
                    Err(_) => break
                }
            }

            thread::sleep(Duration::from_millis(50));
        }
    }
    
    pub fn status(&self) -> String {
        let player_sample: Vec<ServerPingPlayer> = Vec::new();

        // TODO: add sample players to player_sample


        let status_object = ServerPingResponse {
            version: ServerPingVersion {
                name: String::from(self.version),
                protocol: PROTOCOL_VERSION
            },
            players: ServerPingPlayersList {
                max: self.config.max_players,
                online: self.client_list.len() as u16,
                sample: player_sample
            },
            description: ServerPingChatObject {
                text: self.config.motd.to_owned()
            }
        };
        serde_json::to_string(&status_object).unwrap()
    }
}

impl<'a> Drop for QuartzServer<'a> {
    fn drop(&mut self) {
        for (thread_name, handle) in self.join_handles.drain() {
            info!("Shutting down {}", thread_name);
            if let Err(_) = handle.join() {
                error!("Failed to join {}", thread_name);
            }
        }
    }
}

#[derive(Clone)]
struct Client {
    connection: Arc<Mutex<WriteHandle>>,
    player_id: Option<usize>
}

impl Client {
    pub fn new(connection: WriteHandle) -> Self {
        Client {
            connection: Arc::new(Mutex::new(connection)),
            player_id: None
        }
    }

    // Note: blocks the thread
    pub fn send_packet(&self, packet: ClientBoundPacket) {
        // WriteHandle#send_packet should not panic unless there server is already in an unrecoverable state
        self.connection.lock().unwrap().send_packet(packet);
    }
}

#[derive(Serialize)]
struct ServerPingResponse {
    version: ServerPingVersion,
    players: ServerPingPlayersList,
    description: ServerPingChatObject
}

#[derive(Serialize)]
struct ServerPingVersion {
    name: String,
    protocol: i32
}

#[derive(Serialize)]
struct ServerPingPlayersList {
    max: u16,
    online: u16,
    sample: Vec<ServerPingPlayer>
}

#[derive(Serialize)]
struct ServerPingPlayer {
    name: String,
    id: String
}

#[derive(Serialize)]
struct ServerPingChatObject {
    text: String
}
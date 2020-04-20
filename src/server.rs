use std::thread::{self, JoinHandle};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, Duration};

use futures::channel::mpsc::UnboundedReceiver;

use log::*;

use crate::config::Config;
use crate::network::packet_handler::{WrappedServerPacket, ClientBoundPacket, dispatch_sync_packet};
use crate::network::connection::WriteHandle;
use crate::util::ioutil::ByteBuffer;

pub const VERSION: &str = "1.15.2";

static RUNNING: AtomicBool = AtomicBool::new(false);

pub fn is_running() -> bool {
    RUNNING.load(Ordering::SeqCst)
}

pub struct QuartzServer {
    pub config: Config,
    pub client_list: HashMap<usize, Client>,
    sync_packet_receiver: UnboundedReceiver<WrappedServerPacket>,
    join_handles: HashMap<String, JoinHandle<()>>
}

impl QuartzServer {
    pub fn new(config: Config, sync_packet_receiver: UnboundedReceiver<WrappedServerPacket>) -> Self {
        if RUNNING.compare_and_swap(false, true, Ordering::SeqCst) {
            panic!("Attempted to create a server instance after one was already created.");
        }

        QuartzServer {
            config,
            client_list: HashMap::new(),
            sync_packet_receiver,
            join_handles: HashMap::new()
        }
    }

    pub fn init(&mut self) {
        // In case it's needed later
    }

    pub fn add_join_handle(&mut self, thread_name: &str, handle: JoinHandle<()>) {
        self.join_handles.insert(thread_name.to_owned(), handle);
    }

    pub fn send_packet(&self, client_id: usize, packet: ClientBoundPacket) {
        match self.client_list.get(&client_id) {
            Some(client) => client.send_packet(packet),
            None => error!("Could not find client with ID {}, failed to send packet", client_id)
        }
    }

    // This should be used ONLY for the legacy ping response
    pub fn send_buffer(&self, client_id: usize, buffer: ByteBuffer) {
        match self.client_list.get(&client_id) {
            Some(client) => client.send_buffer(buffer),
            None => error!("Could not fine client with ID {}, failed to send buffer", client_id)
        }
    }

    pub fn run(&mut self) {
        info!("Started server thread");

        const FULL_TICK_MILLIS: u128 = 50;
        const FULL_TICK: Duration = Duration::from_millis(FULL_TICK_MILLIS as u64);
        let mut time: SystemTime;

        while RUNNING.load(Ordering::Relaxed) {
            time = SystemTime::now();

            self.tick();

            match time.elapsed() {
                Ok(duration) => {
                    if duration.as_millis() < FULL_TICK_MILLIS {
                        thread::sleep(FULL_TICK - duration);
                    }
                },
                Err(_) => thread::sleep(FULL_TICK)
            }
        }
    }

    fn tick(&mut self) {
        self.handle_packets();
    }

    fn handle_packets(&mut self) {
        loop {
            match self.sync_packet_receiver.try_next() {
                Ok(packet_wrapper) => {
                    match packet_wrapper {
                        Some(packet) => {
                            dispatch_sync_packet(packet, self);
                        },
                        None => break
                    }
                },
                Err(_) => break
            }
        }
    }
}

impl<'a> Drop for QuartzServer {
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
pub struct Client {
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

    // Note: blocks the thread
    // This should ONLY be used for the legacy ping response
    pub fn send_buffer(&self, buffer: ByteBuffer) {
        self.connection.lock().unwrap().send_buffer(buffer);
    }
}
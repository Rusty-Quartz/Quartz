use std::thread::{self, JoinHandle};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
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
    pub client_list: ClientList,
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
            client_list: ClientList::new(),
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
                            dispatch_sync_packet(&packet, self);
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

#[repr(transparent)]
pub struct ClientList(Arc<Mutex<HashMap<usize, Client>>>);

impl ClientList {
    pub fn new() -> Self {
        ClientList(Arc::new(Mutex::new(HashMap::new())))
    }

    fn lock(&self) -> MutexGuard<HashMap<usize, Client>> {
        match self.0.lock() {
            Ok(guard) => guard,
            Err(_) => panic!("Client list mutex poisoned.")
        }
    }

    pub fn add_client(&self, client_id: usize, connection: WriteHandle) {
        self.lock().insert(client_id, Client::new(connection));
    }

    pub fn remove_client(&self, client_id: usize) {
        self.lock().remove(&client_id);
    }

    pub fn online_count(&self) -> usize {
        self.lock().iter().map(|(_id, client)| client.player_id).flatten().count()
    }

    pub fn send_packet(&self, client_id: usize, packet: &ClientBoundPacket) {
        match self.lock().get_mut(&client_id) {
            Some(client) => client.connection.send_packet(packet),
            None => warn!("Attempted to send packet to disconnected client.")
        }
    }

    pub fn send_buffer(&self, client_id: usize, buffer: &ByteBuffer) {
        match self.lock().get_mut(&client_id) {
            Some(client) => client.connection.send_buffer(buffer),
            None => warn!("Attempted to send buffer to disconnected client.")
        }
    }
}

impl Clone for ClientList {
    fn clone(&self) -> Self {
        ClientList(self.0.clone())
    }
}

struct Client {
    pub connection: WriteHandle,
    pub player_id: Option<usize>
}

impl Client {
    pub fn new(connection: WriteHandle) -> Self {
        Client {
            connection,
            player_id: None
        }
    }
}
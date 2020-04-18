use futures::channel::mpsc::UnboundedReceiver;
use std::thread::JoinHandle;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use crate::config::Config;
use crate::network::packet_handler::WrappedServerPacket;

use log::{info, error};

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
) -> &'static mut QuartzServer {
    unsafe {
        match SERVER_STATE.compare_and_swap(UNITIALIZED, INITIALIZING, Ordering::SeqCst) {
            UNITIALIZED => {
                // Initialize the server
                let mut server = QuartzServer {
                    config,
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

#[inline]
pub fn get_server() -> &'static QuartzServer {
    unsafe {
        match SERVER_STATE.load(Ordering::SeqCst) {
            INITIALIZED => SERVER_INSTANCE.as_ref().unwrap(),
            UNITIALIZED | INITIALIZING => panic!("Attempted to access server object before it was initialized."),
            SHUTDOWN => panic!("Attempted to access server object after shutdown."),
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

pub struct QuartzServer {
    pub config: Config,
    sync_packet_receiver: UnboundedReceiver<WrappedServerPacket>,
    join_handles: HashMap<String, JoinHandle<()>>,
    pub version: &'static str
}

impl QuartzServer {
    fn init(&mut self) {
        // In case it's needed later
    }

    pub fn add_join_handle(&mut self, thread_name: &str, handle: JoinHandle<()>) {
        self.join_handles.insert(thread_name.to_owned(), handle);
    }
}

impl Drop for QuartzServer {
    fn drop(&mut self) {
        for (thread_name, handle) in self.join_handles.drain() {
            info!("Shutting down {}", thread_name);
            if let Err(_) = handle.join() {
                error!("Failed to join {}", thread_name);
            }
        }
    }
}
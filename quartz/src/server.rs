use std::collections::HashMap;
use std::error::Error;
use std::io;
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::{
    Arc,
    Mutex,
    MutexGuard,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Sender, Receiver}
};
use std::thread::{self, JoinHandle};
use std::time::{SystemTime, Duration};
use linefeed::{
    Interface,
    DefaultTerminal,
    ReadResult,
    complete::{
        Completer,
        Completion,
        Suffix
    },
    prompter::Prompter
};
use log::*;
use openssl::rsa::Rsa;
use crate::block::init_blocks;
use crate::item::init_items;
use crate::config::Config;
use crate::network::*;
use crate::network::PacketBuffer;
use crate::command::*;
use quartz_plugins::PluginManager;

pub const VERSION: &str = "1.16.1";

pub static RUNNING: AtomicBool = AtomicBool::new(false);

pub fn is_running() -> bool {
    RUNNING.load(Ordering::SeqCst)
}

pub struct QuartzServer {
    pub config: Config,
    pub client_list: ClientList,
    pub console_interface: Arc<Interface<DefaultTerminal>>,
    sync_packet_sender: Sender<WrappedServerPacket>,
    sync_packet_receiver: Receiver<WrappedServerPacket>,
    join_handles: HashMap<String, JoinHandle<()>>,
    pub command_executor: CommandExecutor,
    pub clock: ServerClock,
    pub plugin_manager: PluginManager
}

impl QuartzServer {
    pub fn new(
        config: Config,
        console_interface: Arc<Interface<DefaultTerminal>>
    ) -> Self {
        if RUNNING.compare_and_swap(false, true, Ordering::SeqCst) {
            panic!("Attempted to create a server instance after one was already created.");
        }

        let (sender, receiver) = mpsc::channel::<WrappedServerPacket>();

        QuartzServer {
            config,
            client_list: ClientList::new(),
            console_interface,
            sync_packet_sender: sender,
            sync_packet_receiver: receiver,
            join_handles: HashMap::new(),
            command_executor: CommandExecutor::new(),
            clock: ServerClock::new(50),
            plugin_manager: PluginManager::new(Path::new("./plguins"))
        }
    }

    pub fn init(&mut self) {      
        // Register all of the things
        init_blocks();
        init_items();
        init_commands(&mut self.command_executor);

        // Setup the command handler thread
        self.init_command_handler();

        // Setup TCP server
        if let Err(e) = self.start_tcp_server() {
            error!("Failed to start TCP server: {}", e);
            RUNNING.store(false, Ordering::SeqCst);
        }
    }

    fn init_command_handler(&mut self) {
        let interface = self.console_interface.clone();

        // A simple tab-completer for console
        struct ConsoleCompleter {
            packet_pipe: Mutex<Sender<WrappedServerPacket>>
        };

        impl Completer<DefaultTerminal> for ConsoleCompleter {
            fn complete(
                &self,
                _word: &str,
                prompter: &Prompter<DefaultTerminal>,
                _start: usize,
                _end: usize
            ) -> Option<Vec<Completion>> {
                // Retrieve the packet pipe
                let pipe = match self.packet_pipe.lock() {
                    Ok(pipe) => pipe,
                    Err(_) => return None
                };

                // Build pipes to transfer the completions
                let (sender, receiver) = mpsc::channel::<Vec<String>>();

                // Send the completion request
                pipe.send(WrappedServerPacket::new(0, ServerBoundPacket::HandleConsoleCompletion {
                    // Take the slice of the command up to the cursor
                    command: prompter.buffer()[..prompter.cursor()].to_owned(),
                    response: sender
                })).ok()?;

                // Get the completion response
                receiver.recv().ok().map(|completions| {
                    completions.into_iter()
                        .map(|completion| Completion {
                            completion,
                            display: None,
                            suffix: Suffix::Some(' ')
                        })
                        .collect()
                })
            }
        }

        // Register the tab completer
        interface.set_completer(Arc::new(ConsoleCompleter {packet_pipe: Mutex::new(self.sync_packet_sender.clone())}));

        // Drive the command reader
        let packet_pipe = self.sync_packet_sender.clone();
        self.add_join_handle("Console Command Reader", thread::spawn(move || {
            while RUNNING.load(Ordering::Relaxed) {
                // Check for a new command every 50ms
                match interface.read_line_step(Some(Duration::from_millis(50))) {
                    Ok(result) => match result {
                        Some(ReadResult::Input(command)) => {
                            interface.add_history_unique(command.clone());

                            // Forward the command to the server thread
                            let packet = WrappedServerPacket::new(0, ServerBoundPacket::HandleConsoleCommand {
                                command: command.trim().to_owned()
                            });
                            if let Err(e) = packet_pipe.send(packet) {
                                error!("Failed to forward console command to server thread: {}", e);
                            }
                        },
                        _ => {}
                    },
                    Err(e) => error!("Failed to read console input: {}", e)
                }
            }
        }));
    }

    fn start_tcp_server(&mut self) -> Result<(), Box<dyn Error>> {
        let listener = TcpListener::bind(format!("{}:{}", self.config.server_ip, self.config.port))?;
        if cfg!(target_os = "linux") {
            info!("Running on linux, setting tcp listener to nonblocking");
            listener.set_nonblocking(true)?;
        } else {
            info!("Running on windows, setting tcp listener to blocking");
            listener.set_nonblocking(false)?;
        }

        let sync_packet_sender = self.sync_packet_sender.clone();
        let client_list = self.client_list.clone();

        self.add_join_handle("TCP Server Thread", thread::spawn(move || {
            let mut next_connection_id: usize = 0;
    
            info!("Started TCP Server Thread");
    
            // If this fails a panic is justified
            let key_pair = Arc::new(Rsa::generate(1024).unwrap());
    
            loop {
                match listener.accept() {
                    // Successful connection
                    Ok((socket, _addr)) => {
                       // Don't bother handling the connection if the server is shutting down
                        if !RUNNING.load(Ordering::SeqCst) {
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
                        if RUNNING.load(Ordering::SeqCst) {
                            thread::sleep(Duration::from_millis(100));
                        } else {
                            return;
                        }
                    },
    
                   // Actual error
                    Err(e) => error!("Failed to accept TCP socket: {}", e)
                };
            }
        }));

        Ok(())
    }

    pub fn add_join_handle(&mut self, thread_name: &str, handle: JoinHandle<()>) {
        self.join_handles.insert(thread_name.to_owned(), handle);
    }

    pub fn run(&mut self) {
        info!("Started server thread");

        while RUNNING.load(Ordering::Relaxed) {
            self.clock.start();
            self.tick();
            self.clock.finish_tick();
        }
    }

    fn tick(&mut self) {
        self.handle_packets();
    }

    fn handle_packets(&mut self) {
        while let Ok(packet) = self.sync_packet_receiver.try_recv() {
            dispatch_sync_packet(&packet, self);
        }
    }
}

impl Drop for QuartzServer {
    fn drop(&mut self) {
        // In case this is reached due to a panic
        RUNNING.store(false, Ordering::SeqCst);

        for (thread_name, handle) in self.join_handles.drain() {
            info!("Shutting down {}", thread_name);

            if thread_name == "TCP Server Thread" {
                if cfg!(target_os = "windows") {
                    debug!("sending stupid connection to listener to close it");
                    TcpStream::connect(format!("{}:{}", self.config.server_ip, self.config.port)).unwrap();
                }
            }

            if let Err(_) = handle.join() {
                error!("Failed to join {}", thread_name);
            }
        }
    }
}

// Keeps track of the time each tick takes and regulates the server TPS
pub struct ServerClock {
    micros_ema: f32,
    full_tick_millis: u128,
    full_tick: Duration,
    time: SystemTime
}

impl ServerClock {
    pub fn new(tick_length: u128) -> Self {
        ServerClock {
            micros_ema: 0_f32,
            full_tick_millis: tick_length,
            full_tick: Duration::from_millis(tick_length as u64),
            time: SystemTime::now()
        }
    }

    // Start of a new tick
    pub fn start(&mut self) {
        self.time = SystemTime::now();
    }

    // The tick code has finished executing, so record the time and sleep if extra time remains
    pub fn finish_tick(&mut self) {
        match self.time.elapsed() {
            Ok(duration) => {
                self.micros_ema = (99_f32 * self.micros_ema + duration.as_micros() as f32) / 100_f32;

                if duration.as_millis() < self.full_tick_millis {
                    thread::sleep(self.full_tick - duration);
                }
            },
            Err(_) => thread::sleep(self.full_tick)
        }
    }

    // Milliseconds per tick
    #[inline]
    pub fn mspt(&self) -> f32 {
        self.micros_ema / 1000_f32
    }

    // Convert mspt to tps
    #[inline]
    pub fn as_tps(&self, mspt: f32) -> f32 {
        if mspt < self.full_tick_millis as f32 {
            1000_f32 / (self.full_tick_millis as f32)
        } else {
            1000_f32 / mspt
        }
    }

    // The maximum tps the server will tick at
    #[inline]
    pub fn max_tps(&self) -> f32 {
        1000_f32 / self.full_tick_millis as f32
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

    pub fn send_buffer(&self, client_id: usize, buffer: &PacketBuffer) {
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
use crate::{
    command::{CommandContext, CommandModule, CommandSender},
    command_executor,
    config,
    display_to_console,
    item::init_items,
    network::*,
    raw_console,
    world::chunk::ChunkProvider,
    Registry,
    RUNNING,
};
use linefeed::{
    complete::{Completer, Completion, Suffix},
    prompter::Prompter,
    DefaultTerminal,
    ReadResult,
    Signal,
};
use log::*;
use openssl::rsa::Rsa;
use parking_lot::Mutex;
use quartz_net::*;
use rand::{thread_rng, Rng};
use std::{
    collections::HashMap,
    error::Error,
    net::TcpStream as StdTcpStream,
    process::abort,
    sync::{
        atomic::Ordering,
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tokio::{
    net::TcpListener,
    runtime::{Builder, Runtime},
    task,
};

/// The string form of the minecraft version quartz currently supports.
pub const VERSION: &str = "1.17";

/// The main struct containing all relevant data to the quartz server instance.
// TODO: consider boxing some fields
pub struct QuartzServer {
    /// The list of connected clients.
    pub(crate) client_list: ClientList,
    /// The join handle for the console command handler thread.
    console_command_handler: Option<JoinHandle<()>>,
    /// The ChunckProvider
    pub chunk_provider: ChunkProvider,
    tcp_server_runtime: Option<Runtime>,
    /// A cloneable channel to send packets to the main server thread.
    sync_packet_sender: Sender<WrappedServerBoundPacket>,
    /// The receiver for packets that need to be handled on the server thread.
    sync_packet_receiver: Receiver<WrappedServerBoundPacket>,
}

impl QuartzServer {
    pub(crate) fn new() -> Self {
        if RUNNING
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            panic!("Attempted to create a server instance after one was already created.");
        }

        let (sender, receiver) = mpsc::channel::<WrappedServerBoundPacket>();
        let tcp_server_runtime = Builder::new_multi_thread()
            .enable_io()
            .thread_name("tcp-worker")
            .build()
            .map(Some)
            .expect("Failed to construct TCP server runtime");

        QuartzServer {
            client_list: ClientList::new(),
            sync_packet_sender: sender,
            sync_packet_receiver: receiver,
            console_command_handler: None,
            chunk_provider: ChunkProvider::new("world", "./world/region")
                .expect("Error making chunk provider"),
            tcp_server_runtime,
        }
    }

    /// Initializes the server. This loads all blocks, items, and other game data, initializes commands
    /// and plugins, and starts the TCP server.
    pub fn init(&mut self) {
        RUNNING.store(self.init_internal(), Ordering::Release);
    }

    fn init_internal(&mut self) -> bool {
        // Register all of the things
        init_items();

        // Initialize commands
        // TODO: deal with dynamically added commands

        // Setup the command handler thread
        self.init_command_handler();

        // Setup TCP server
        if let Err(e) = self.start_tcp_server() {
            error!("Failed to start TCP server: {}", e);
            return false;
        }

        match Registry::init() {
            Ok(()) => true,
            Err(()) => {
                error!("Failed to initialize registry. Was it already initialized?");
                false
            }
        }
    }

    fn init_command_handler(&mut self) {
        // A simple tab-completer for console
        struct ConsoleCompleter {
            packet_pipe: Mutex<Sender<WrappedServerBoundPacket>>,
        }

        impl Completer<DefaultTerminal> for ConsoleCompleter {
            fn complete(
                &self,
                _word: &str,
                prompter: &Prompter<'_, '_, DefaultTerminal>,
                _start: usize,
                _end: usize,
            ) -> Option<Vec<Completion>> {
                // Retrieve the packet pipe
                let pipe = self.packet_pipe.lock();

                // Build pipes to transfer the completions
                let (sender, receiver) = mpsc::channel::<Vec<String>>();

                // Send the completion request
                pipe.send(WrappedServerBoundPacket::ConsoleCompletion {
                    command: prompter.buffer()[.. prompter.cursor()].to_owned(),
                    response: sender,
                })
                .ok()?;

                // Get the completion response
                receiver.recv().ok().map(|completions| {
                    completions
                        .into_iter()
                        .map(|completion| Completion {
                            completion,
                            display: None,
                            suffix: if prompter.cursor() == prompter.buffer().len() {
                                Suffix::Some(' ')
                            } else {
                                Suffix::None
                            },
                        })
                        .collect()
                })
            }
        }

        // Set the console completer
        raw_console().set_completer(Arc::new(ConsoleCompleter {
            packet_pipe: Mutex::new(self.sync_packet_sender.clone()),
        }));

        // Drive the command reader
        let packet_pipe = self.sync_packet_sender.clone();
        let handle = thread::spawn(move || {
            let interface = raw_console();

            while RUNNING.load(Ordering::Acquire) {
                // Check for a new command every 50ms
                match interface.read_line_step(Some(Duration::from_millis(50))) {
                    Ok(result) => match result {
                        Some(ReadResult::Input(command)) => {
                            if command == "abort" {
                                // There are lots of opportunities for accidental deadlocking
                                // which could prevent commands from being processed. This is
                                // a utility for exiting the process if such an event were to
                                // occur.

                                abort();
                            }

                            interface.add_history_unique(command.clone());

                            // Forward the command to the server thread
                            let packet = WrappedServerBoundPacket::ConsoleCommand {
                                command: command.trim().to_owned(),
                            };
                            if let Err(e) = packet_pipe.send(packet) {
                                error!("Failed to forward console command to server thread: {}", e);
                            }
                        }
                        Some(ReadResult::Signal(Signal::Interrupt | Signal::Quit)) => {
                            let _ = packet_pipe.send(WrappedServerBoundPacket::ConsoleCommand {
                                command: "stop".to_owned(),
                            });
                        }
                        _ => {}
                    },
                    Err(e) => error!("Failed to read console input: {}", e),
                }
            }
        });
        self.console_command_handler = Some(handle);
    }

    fn start_tcp_server(&mut self) -> Result<(), Box<dyn Error>> {
        let config = config()
            .try_read()
            .expect("Config locked during initialization.");
        let addr = format!("{}:{}", config.server_ip, config.port);
        drop(config);

        let sync_packet_sender = self.sync_packet_sender.clone();

        let rt = self
            .tcp_server_runtime
            .as_ref()
            .expect("TCP server runtime not initialized");
        let listener = rt.block_on(TcpListener::bind(addr))?;
        rt.spawn(Self::tcp_server(listener, sync_packet_sender));

        Ok(())
    }

    async fn tcp_server(
        listener: TcpListener,
        sync_packet_sender: Sender<WrappedServerBoundPacket>,
    ) {
        let mut next_connection_id: usize = 0;

        info!("Started TCP Server Thread");

        // If this fails a panic is justified
        let key_pair = Arc::new(Rsa::generate(1024).unwrap());

        loop {
            match listener.accept().await {
                // Successful connection
                Ok((socket, _addr)) => {
                    // Don't bother handling the connection if the server is shutting down
                    if !RUNNING.load(Ordering::Acquire) {
                        return;
                    }

                    debug!("Client connected");

                    // Construct a connection wrapper around the socket
                    let (conn, driver) = AsyncClientConnection::new(
                        next_connection_id,
                        socket,
                        sync_packet_sender.clone(),
                    );

                    // Register the client
                    let result =
                        sync_packet_sender.send(WrappedServerBoundPacket::ClientConnected {
                            id: next_connection_id,
                            write_handle: conn.write_handle.clone(),
                        });
                    if let Err(e) = result {
                        error!("Fatal error: failed to register new client: {}", e);
                        return;
                    }

                    task::spawn(driver);

                    // Spawn a thread to handle the connection asynchronously
                    let key_pair_clone = key_pair.clone();
                    task::spawn(async move { handle_async_connection(conn, key_pair_clone).await });

                    next_connection_id += 1;
                }

                // Actual error
                Err(e) => error!("Failed to accept TCP socket: {}", e),
            };
        }
    }

    pub(crate) async fn tick(&mut self) {
        self.handle_packets().await;
        self.client_list.update_keep_alive();
        self.chunk_provider.flush_ready().await;
    }

    async fn handle_packets(&mut self) {
        while let Ok(wrapped_packet) = self.sync_packet_receiver.try_recv() {
            match wrapped_packet {
                WrappedServerBoundPacket::External { sender, ref packet } =>
                    dispatch_sync_packet(sender, packet, self).await,
                WrappedServerBoundPacket::LoginSuccess { id, uuid, username } =>
                    self.handle_login_success_server(id, uuid, &username).await,
                WrappedServerBoundPacket::ClientConnected { id, write_handle } =>
                    self.client_list.add_client(id, write_handle),
                WrappedServerBoundPacket::ClientDisconnected { id } =>
                    self.client_list.remove_client(id),
                WrappedServerBoundPacket::ConsoleCommand { command } => {
                    let executor = command_executor();
                    let sender = CommandSender::Console;
                    let context = CommandContext::new(self, &*executor, sender);
                    if let Err(e) = executor.dispatch(&command, context) {
                        display_to_console(&e);
                    }
                }
                WrappedServerBoundPacket::ConsoleCompletion { command, response } => {
                    let executor = command_executor();
                    let sender = CommandSender::Console;
                    let context = CommandContext::new(self, executor, sender);
                    let suggestions = executor.get_suggestions(&command, &context);
                    // Error handling not useful here
                    drop(response.send(suggestions));
                }
            }
        }
    }
}

impl Drop for QuartzServer {
    fn drop(&mut self) {
        // In case this is reached due to a panic
        RUNNING.store(false, Ordering::Release);

        // Shutdown the command handler
        match self.console_command_handler.take() {
            Some(handle) => {
                info!("Shutting down command handler thread");
                let _ = handle.join();
            }
            None => {}
        }

        // Send a connection to the server daemon to shut it down
        match config().try_read() {
            Some(guard) => {
                let _ = StdTcpStream::connect(format!("{}:{}", guard.server_ip, guard.port));
            }
            None => warn!("Failed to shutdown TCP thread."),
        }

        // Shutdown the server runtime, forcibly stopping it after five seconds
        if let Some(rt) = self.tcp_server_runtime.take() {
            rt.shutdown_timeout(Duration::from_secs(5));
        }
    }
}

/// A thread-safe wrapper around a map of clients and their connection IDs.
#[repr(transparent)]
pub struct ClientList(HashMap<usize, Client>);

impl ClientList {
    /// Creates a new, empty client list.
    pub fn new() -> Self {
        ClientList(HashMap::new())
    }

    /// Adds a new client with the given ID and write handle.
    pub fn add_client(&mut self, client_id: usize, connection: AsyncWriteHandle) {
        self.0.insert(client_id, Client::new(connection));
    }

    /// Removes the client with the given ID.
    pub fn remove_client(&mut self, client_id: usize) {
        self.0.remove(&client_id);
    }

    /// Returns the number of players currently online.
    pub fn online_count(&self) -> usize {
        self.0
            .iter()
            .map(|(_id, client)| client.player_id)
            .flatten()
            .count()
    }

    pub fn create_write_handle(&self, client_id: usize) -> Option<AsyncWriteHandle> {
        self.0
            .get(&client_id)
            .map(|client| client.connection.clone())
    }

    pub fn start_keep_alive(&mut self, client_id: usize) {
        match self.0.get_mut(&client_id) {
            Some(client) => {
                let keep_alive_id = thread_rng().gen();
                client.keep_alive_id = Some(keep_alive_id);
                client.last_keep_alive_exchange = Instant::now();
                client
                    .connection
                    .send_packet(ClientBoundPacket::KeepAlive { keep_alive_id });
            }
            None => warn!("Attempted to start keep-alive chain on a disconnected client."),
        }
    }

    pub fn handle_keep_alive(&mut self, client_id: usize, keep_alive_id: i64) {
        match self.0.get_mut(&client_id) {
            Some(client) =>
                if client.keep_alive_id == Some(keep_alive_id) {
                    client.keep_alive_id = None;
                } else {
                    self.0.remove(&client_id);
                },
            None => warn!("Attempted to handle a keep-alive packet on a disconnected client."),
        }
    }

    pub fn update_keep_alive(&mut self) {
        self.0.retain(|_, client| client.should_keep_alive());
    }

    /// Sends a packet to the client with the given ID.
    pub fn send_packet(&self, client_id: usize, packet: ClientBoundPacket) {
        match self.0.get(&client_id) {
            Some(client) => client.connection.send_packet(packet),
            None => warn!("Attempted to send packet to disconnected client."),
        }
    }

    pub fn send_all<I>(&self, client_id: usize, packets: I)
    where I: IntoIterator<Item = ClientBoundPacket> {
        match self.0.get(&client_id) {
            Some(client) => client.connection.send_all(packets),
            None => warn!("Attempted to send packet to disconnected client."),
        }
    }

    /// Sends a raw byte buffer to the client with the given ID.
    pub fn send_buffer(&self, client_id: usize, buffer: PacketBuffer) {
        match self.0.get(&client_id) {
            Some(client) => client.connection.send_packet(buffer),
            None => warn!("Attempted to send buffer to disconnected client."),
        }
    }
}

/// Wrapper around an asynchronous connection write handle that also contains an optional player ID.
struct Client {
    pub connection: AsyncWriteHandle,
    pub player_id: Option<usize>,
    keep_alive_id: Option<i64>,
    last_keep_alive_exchange: Instant,
}

impl Client {
    pub fn new(connection: AsyncWriteHandle) -> Self {
        Client {
            connection,
            player_id: None,
            keep_alive_id: None,
            last_keep_alive_exchange: Instant::now(),
        }
    }

    fn should_keep_alive(&mut self) -> bool {
        let elapsed = self.last_keep_alive_exchange.elapsed();

        // Do an exchange at a maximum rate of once every five seconds
        if elapsed > Duration::from_secs(5) {
            match self.keep_alive_id {
                // Kick the client if it's been more than 30 seconds since we've heard back
                Some(..) =>
                    if elapsed > Duration::from_secs(30) {
                        return false;
                    },

                // Send another keep-alive packet
                None => {
                    let keep_alive_id = thread_rng().gen();
                    self.keep_alive_id = Some(keep_alive_id);
                    self.connection
                        .send_packet(ClientBoundPacket::KeepAlive { keep_alive_id });
                    self.last_keep_alive_exchange = Instant::now();
                }
            }
        }

        true
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.connection.shutdown();
    }
}

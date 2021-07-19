use crate::{
    config_unchecked,
    item::init_items,
    network::{
        packet::{ClientBoundPacket, ServerBoundPacket, WrappedServerBoundPacket},
        *,
    },
    raw_console_unchecked,
    world::chunk::ChunkProvider,
    Registry,
    RUNNING,
};
use futures_lite::future;
use linefeed::{
    complete::{Completer, Completion, Suffix},
    prompter::Prompter,
    DefaultTerminal,
    ReadResult,
};
use log::*;
use openssl::rsa::Rsa;
use smol::{net::TcpListener, LocalExecutor};
use std::{
    collections::HashMap,
    error::Error,
    net::TcpStream as StdTcpStream,
    rc::Rc,
    sync::{
        atomic::Ordering,
        mpsc::{self, Receiver, Sender},
        Arc,
        Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

/// The string form of the minecraft version quartz currently supports.
pub const VERSION: &str = "1.17";

/// The main struct containing all relevant data to the quartz server instance.
// TODO: consider boxing some fields
pub struct QuartzServer {
    /// The list of connected clients.
    pub(crate) client_list: ClientList,
    /// A cloneable channel to send packets to the main server thread.
    sync_packet_sender: Sender<WrappedServerBoundPacket>,
    /// The receiver for packets that need to be handled on the server thread.
    sync_packet_receiver: Receiver<WrappedServerBoundPacket>,
    /// A map of thread join handles to join when the server is dropped.
    join_handles: HashMap<String, JoinHandle<()>>,
    /// The ChunckProvider
    pub chunk_provider: ChunkProvider,
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

        QuartzServer {
            client_list: ClientList::new(),
            sync_packet_sender: sender,
            sync_packet_receiver: receiver,
            join_handles: HashMap::new(),
            chunk_provider: ChunkProvider::new("world", "./world/region", 4)
                .expect("Error making chunk provider"),
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
                let pipe = match self.packet_pipe.lock() {
                    Ok(pipe) => pipe,
                    Err(_) => return None,
                };

                // Build pipes to transfer the completions
                let (sender, receiver) = mpsc::channel::<Vec<String>>();

                // Send the completion request
                pipe.send(WrappedServerBoundPacket::new(
                    0,
                    ServerBoundPacket::ConsoleCompletion {
                        // Take the slice of the command up to the cursor
                        command: prompter.buffer()[.. prompter.cursor()].to_owned(),
                        response: sender,
                    },
                ))
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

        // Register the tab completer
        // Safety: this code can only be called within this crate,
        unsafe {
            raw_console_unchecked().set_completer(Arc::new(ConsoleCompleter {
                packet_pipe: Mutex::new(self.sync_packet_sender.clone()),
            }));
        }

        // Drive the command reader
        let packet_pipe = self.sync_packet_sender.clone();
        self.add_join_handle(
            "Console Command Reader",
            thread::spawn(move || {
                let interface = unsafe { raw_console_unchecked() };

                while RUNNING.load(Ordering::Acquire) {
                    // Check for a new command every 50ms
                    match interface.read_line_step(Some(Duration::from_millis(50))) {
                        Ok(result) => match result {
                            Some(ReadResult::Input(command)) => {
                                interface.add_history_unique(command.clone());

                                // Forward the command to the server thread
                                let packet = WrappedServerBoundPacket::new(
                                    0,
                                    ServerBoundPacket::ConsoleCommand {
                                        command: command.trim().to_owned(),
                                    },
                                );
                                if let Err(e) = packet_pipe.send(packet) {
                                    error!(
                                        "Failed to forward console command to server thread: {}",
                                        e
                                    );
                                }
                            }
                            _ => {}
                        },
                        Err(e) => error!("Failed to read console input: {}", e),
                    }
                }
            }),
        );
    }

    fn start_tcp_server(&mut self) -> Result<(), Box<dyn Error>> {
        let config = unsafe { config_unchecked() }
            .try_lock()
            .expect("Config locked during initialization.");
        let listener = smol::block_on(TcpListener::bind(format!(
            "{}:{}",
            config.server_ip, config.port
        )))?;
        drop(config);

        let sync_packet_sender = self.sync_packet_sender.clone();

        // TODO: consider adding more threads
        self.add_join_handle(
            "TCP Server Thread",
            thread::spawn(move || {
                let executor = Rc::new(LocalExecutor::new());
                future::block_on(executor.run(Self::tcp_server(
                    executor.clone(),
                    listener,
                    sync_packet_sender,
                )));
            }),
        );

        Ok(())
    }

    async fn tcp_server(
        executor: Rc<LocalExecutor<'static>>,
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
                    let conn = AsyncClientConnection::new(
                        next_connection_id,
                        socket,
                        sync_packet_sender.clone(),
                    );

                    // Register the client
                    let result = sync_packet_sender.send(WrappedServerBoundPacket::new(
                        0,
                        ServerBoundPacket::ClientConnected {
                            id: next_connection_id,
                            write_handle: conn.create_write_handle(),
                        },
                    ));
                    if let Err(e) = result {
                        error!("Fatal error: failed to register new client: {}", e);
                        return;
                    }

                    // Spawn a thread to handle the connection asynchronously
                    let key_pair_clone = key_pair.clone();
                    executor
                        .spawn(async move { handle_async_connection(conn, key_pair_clone).await })
                        .detach();

                    next_connection_id += 1;
                }

                // Actual error
                Err(e) => error!("Failed to accept TCP socket: {}", e),
            };
        }
    }

    /// Registers a join handle to be joined when the server is dropped.
    fn add_join_handle(&mut self, thread_name: &str, handle: JoinHandle<()>) {
        self.join_handles.insert(thread_name.to_owned(), handle);
    }

    pub(crate) async fn tick(&mut self) {
        self.handle_packets().await;
        self.chunk_provider.flush_queue().await;
    }

    async fn handle_packets(&mut self) {
        while let Ok(wrapped_packet) = self.sync_packet_receiver.try_recv() {
            match wrapped_packet.packet {
                ServerBoundPacket::ClientConnected { id, write_handle } =>
                    self.client_list.add_client(id, write_handle),
                ServerBoundPacket::ClientDisconnected { id } => self.client_list.remove_client(id),
                _ => dispatch_sync_packet(&wrapped_packet, self).await,
            }
        }
    }
}

impl Drop for QuartzServer {
    fn drop(&mut self) {
        // In case this is reached due to a panic
        RUNNING.store(false, Ordering::Release);

        // Send a connection to the server daemon to shut it down
        match unsafe { config_unchecked() }.try_lock() {
            Some(guard) => {
                let _ = StdTcpStream::connect(format!("{}:{}", guard.server_ip, guard.port));
            }
            None => warn!("Failed to shutdown TCP thread."),
        }

        for (thread_name, handle) in self.join_handles.drain() {
            info!("Shutting down {}", thread_name);

            if let Err(_) = handle.join() {
                error!("Failed to join {}", thread_name);
            }
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

    /// Sends a packet to the client with the given ID.
    pub async fn send_packet(&mut self, client_id: usize, packet: ClientBoundPacket) {
        match self.0.get_mut(&client_id) {
            Some(client) => client.connection.send_packet(packet).await,
            None => warn!("Attempted to send packet to disconnected client."),
        }
    }

    /// Sends a raw byte buffer to the client with the given ID.
    pub async fn send_buffer(&mut self, client_id: usize, buffer: PacketBuffer) {
        match self.0.get_mut(&client_id) {
            Some(client) => client.connection.send_buffer(buffer).await,
            None => warn!("Attempted to send buffer to disconnected client."),
        }
    }
}

/// Wrapper around an asynchronous connection write handle that also contains an optional player ID.
struct Client {
    pub connection: AsyncWriteHandle,
    pub player_id: Option<usize>,
}

impl Client {
    pub fn new(connection: AsyncWriteHandle) -> Self {
        Client {
            connection,
            player_id: None,
        }
    }
}

use crate::{
    command::{CommandContext, CommandModule, CommandSender},
    command_executor,
    config,
    display_to_console,
    item::init_items,
    network::*,
    raw_console,
    world::world::WorldStore,
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
use quartz_chat::{
    color::Color,
    component::{ClickEvent, ComponentType, HoverEntity, HoverEvent},
    Component,
    ComponentBuilder,
};
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
use tokio::{net::TcpListener, runtime::Runtime, task};
use uuid::Uuid;

/// The string form of the minecraft version quartz currently supports.
pub const VERSION: &str = "1.17";

/// The main struct containing all relevant data to the quartz server instance.
// TODO: consider boxing some fields
pub struct QuartzServer {
    rt: Arc<Runtime>,
    /// The list of connected clients.
    pub(crate) client_list: ClientList,
    /// The join handle for the console command handler thread.
    console_command_handler: Option<JoinHandle<()>>,
    ///The World manager
    pub world_store: WorldStore,
    /// A cloneable channel to send packets to the main server thread.
    sync_packet_sender: Sender<WrappedServerBoundPacket>,
    /// The receiver for packets that need to be handled on the server thread.
    sync_packet_receiver: Receiver<WrappedServerBoundPacket>,
}

impl QuartzServer {
    pub(crate) fn new(rt: Arc<Runtime>) -> Self {
        if RUNNING
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            panic!("Attempted to create a server instance after one was already created.");
        }

        let (sender, receiver) = mpsc::channel::<WrappedServerBoundPacket>();
        let world_store =
            WorldStore::new(Arc::clone(&rt), "./world").expect("Error making world store");

        QuartzServer {
            rt,
            client_list: ClientList::new(),
            sync_packet_sender: sender,
            sync_packet_receiver: receiver,
            console_command_handler: None,
            world_store,
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

        let listener = self.rt.block_on(TcpListener::bind(addr))?;
        self.rt
            .spawn(Self::tcp_server(listener, sync_packet_sender));

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
        self.world_store.flush_ready().await;
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
                WrappedServerBoundPacket::ClientDisconnected { id } => {
                    self.client_list.remove_client(id);
                    if let Err(_e) = self.world_store.remove_player(id).await {
                        // Only log this error in debug mode
                        // It can be an error but also triggers when sending status packets
                        // so in release we probably shouldn't log it
                        // but it could be useful to mark as error when debugging
                        #[cfg(debug_assertations)]
                        log::error!("Error removing player: {}", e);
                    }
                }
                WrappedServerBoundPacket::ConsoleCommand { command } => {
                    let executor = command_executor();
                    let sender = CommandSender::Console;
                    let context = CommandContext::new(self, executor, sender);
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
        if let Some(handle) = self.console_command_handler.take() {
            info!("Shutting down command handler thread");
            let _ = handle.join();
        }

        // Send a connection to the server daemon to shut it down
        match config().try_read() {
            Some(guard) => {
                let _ = StdTcpStream::connect(format!("{}:{}", guard.server_ip, guard.port));
            }
            None => warn!("Failed to shutdown TCP thread."),
        }
    }
}

pub type ClientId = usize;

/// A thread-safe wrapper around a map of clients and their connection IDs.
#[repr(transparent)]
pub struct ClientList(HashMap<ClientId, Client>);

impl ClientList {
    /// Creates a new, empty client list.
    pub fn new() -> Self {
        ClientList(HashMap::new())
    }

    /// Adds a new client with the given ID and write handle.
    pub fn add_client(&mut self, client_id: ClientId, connection: AsyncWriteHandle) {
        self.0.insert(client_id, Client::new(connection));
    }

    /// Removes the client with the given ID.
    pub fn remove_client(&mut self, client_id: ClientId) {
        self.0.remove(&client_id);
    }

    /// Returns the number of players currently online.
    pub fn online_count(&self) -> usize {
        self.0
            .iter()
            .filter_map(|(_id, client)| client.player_id)
            .count()
    }

    pub fn create_write_handle(&self, client_id: ClientId) -> Option<AsyncWriteHandle> {
        self.0
            .get(&client_id)
            .map(|client| client.connection.clone())
    }

    pub fn start_keep_alive(&mut self, client_id: ClientId) {
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

    pub fn handle_keep_alive(&mut self, client_id: ClientId, keep_alive_id: i64) {
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
    pub fn send_packet(&self, client_id: ClientId, packet: ClientBoundPacket) {
        match self.0.get(&client_id) {
            Some(client) => client.connection.send_packet(packet),
            None => warn!("Attempted to send packet to disconnected client."),
        }
    }

    pub fn send_all<I>(&self, client_id: ClientId, packets: I)
    where I: IntoIterator<Item = ClientBoundPacket> {
        match self.0.get(&client_id) {
            Some(client) => client.connection.send_all(packets),
            None => warn!("Attempted to send packet to disconnected client."),
        }
    }

    /// Sends a raw byte buffer to the client with the given ID.
    pub fn send_buffer(&self, client_id: ClientId, buffer: PacketBuffer) {
        match self.0.get(&client_id) {
            Some(client) => client.connection.send_packet(buffer),
            None => warn!("Attempted to send buffer to disconnected client."),
        }
    }

    /// Sends a packet to every client connected
    pub fn send_to_all<P>(&self, packet: P)
    where P: Fn(&ClientId) -> ClientBoundPacket {
        self.iter()
            .for_each(|(id, client)| client.connection.send_packet(packet(id)));
    }

    /// Sends a packet to every client that passes the provided filter
    pub fn send_to_filtered<F, P>(&self, packet: P, filter: F)
    where
        F: Fn(&&ClientId) -> bool,
        P: Fn(&ClientId) -> ClientBoundPacket,
    {
        self.iter()
            .filter(|(id, _client)| filter(id))
            .for_each(|(id, client)| client.connection.send_packet(packet(id)));
    }

    pub fn iter(&self) -> ClientListIter<'_> {
        ClientListIter(self.0.iter())
    }

    pub fn set_username(&mut self, client_id: ClientId, username: &str) -> Option<()> {
        self.0.get_mut(&client_id)?.username = username.to_string();
        Some(())
    }

    pub fn set_uuid(&mut self, client_id: ClientId, uuid: Uuid) -> Option<()> {
        self.0.get_mut(&client_id)?.uuid = uuid;
        Some(())
    }

    pub fn username(&self, client_id: ClientId) -> Option<&str> {
        Some(self.0.get(&client_id)?.username())
    }

    pub fn uuid(&self, client_id: ClientId) -> Option<&Uuid> {
        Some(self.0.get(&client_id)?.uuid())
    }

    pub fn send_chat(&self, client_id: ClientId, message: &str) -> Option<()> {
        let uuid = *self.uuid(client_id)?;
        let username = self.username(client_id)?;

        self.iter()
            .for_each(|(_, c)| c.send_message(message, Some((uuid, username)), false));
        Some(())
    }

    pub fn send_system_message(&self, client_id: ClientId, message: &str) -> Option<()> {
        self.0.get(&client_id)?.send_message(message, None, false);
        Some(())
    }

    pub fn send_system_error(&self, client_id: ClientId, message: &str) -> Option<()> {
        self.0.get(&client_id)?.send_message(message, None, true);
        Some(())
    }
}

pub struct ClientListIter<'a>(std::collections::hash_map::Iter<'a, ClientId, Client>);

impl<'a> Iterator for ClientListIter<'a> {
    type Item = (&'a ClientId, &'a Client);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl Default for ClientList {
    fn default() -> Self {
        Self::new()
    }
}

/// Wrapper around an asynchronous connection write handle that also contains an optional player ID.
pub struct Client {
    pub connection: AsyncWriteHandle,
    pub player_id: Option<usize>,
    keep_alive_id: Option<i64>,
    last_keep_alive_exchange: Instant,
    username: String,
    // The minecraft uuid of the client
    uuid: Uuid,
}

impl Client {
    pub fn new(connection: AsyncWriteHandle) -> Self {
        Client {
            connection,
            player_id: None,
            keep_alive_id: None,
            last_keep_alive_exchange: Instant::now(),
            username: Default::default(),
            uuid: Uuid::default(),
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

    pub fn username(&self) -> &str {
        &self.username
    }

    pub fn uuid(&self) -> &Uuid {
        &self.uuid
    }

    fn send_message(&self, message: &str, user_info: Option<(Uuid, &str)>, sys_error: bool) {
        match user_info {
            Some((uuid, username)) => self.connection.send_packet(ClientBoundPacket::ChatMessage {
                sender: uuid,
                position: 0,
                json_data: Box::new(Component {
                    component_type: ComponentType::translate(
                        "chat.type.text".to_owned(),
                        Some(vec![
                            ComponentBuilder::empty()
                                .click_event(ClickEvent::suggest_command(format!(
                                    "/tell {} ",
                                    username
                                )))
                                .hover_event(HoverEvent::show_entity(HoverEntity {
                                    id: uuid.to_string(),
                                    name: Some(Component::text(username)),
                                    entity_type: Some("minecraft:player".to_owned()),
                                }))
                                .insertion(username.to_owned())
                                .add_text(username)
                                .build(),
                            Component::text(message),
                        ]),
                    ),
                    color: if sys_error { Some(Color::Red) } else { None },
                    ..Default::default()
                }),
            }),
            None => self.connection.send_packet(ClientBoundPacket::ChatMessage {
                sender: Uuid::from_u128(0),
                position: 1,
                json_data: Box::new(Component::text(message)),
            }),
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.connection.shutdown();
    }
}

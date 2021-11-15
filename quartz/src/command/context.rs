use crate::{display_to_console, network::AsyncWriteHandle, CommandExecutor, QuartzServer};
use quartz_chat::component::Component;
use quartz_net::ClientBoundPacket;
use uuid::Uuid;

/// The context in which a command is executed. This has no use outside the lifecycle of a command.
pub struct CommandContext<'ctx> {
    /// A shared reference to the server.
    pub server: &'ctx mut QuartzServer,
    /// A shared reference to the executor that created this context.
    pub executor: &'ctx CommandExecutor,
    /// The sender of the command.
    pub sender: CommandSender,
}

// Shortcut functions for getting argument values
impl<'ctx> CommandContext<'ctx> {
    /// Creates a new command context with the given parameters.
    pub fn new(
        server: &'ctx mut QuartzServer,
        executor: &'ctx CommandExecutor,
        sender: CommandSender,
    ) -> Self {
        CommandContext {
            server,
            executor,
            sender,
        }
    }
}

/// A command sender, can be command block, player, or the console.
pub enum CommandSender {
    /// The console sender type.
    Console,
    Client(AsyncWriteHandle),
}

impl CommandSender {
    /// Sends a message to the sender.
    pub fn send_message(&self, message: Component) {
        match self {
            CommandSender::Console => display_to_console(&message),
            CommandSender::Client(handle) => handle.send_packet(ClientBoundPacket::ChatMessage {
                sender: Uuid::from_u128(0),
                position: 1,
                json_data: Box::new(message),
            }),
        }
    }
}

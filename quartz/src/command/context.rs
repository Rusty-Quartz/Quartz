use crate::{CommandExecutor, QuartzServer};
use chat::component::Component;
use linefeed::{terminal::DefaultTerminal, Interface};
use log::error;
use std::sync::Arc;

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
    Console(
        /// A handle to log messages to console.
        Arc<Interface<DefaultTerminal>>,
    ),
}

impl CommandSender {
    /// Sends a message to the sender.
    pub fn send_message(&self, message: Component) {
        match self {
            CommandSender::Console(interface) => match interface.lock_writer_erase() {
                Ok(mut writer) =>
                    if let Err(e) = writeln!(writer, "{}", message) {
                        error!("Failed to send message to console: {}", e);
                    },
                Err(e) => error!("Failed to lock console interface: {}", e),
            },
        }
    }
}

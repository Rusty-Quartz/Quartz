use chat::component::Component;
use linefeed::{terminal::DefaultTerminal, Interface};
use log::error;
use std::sync::Arc;

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

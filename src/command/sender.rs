use std::sync::Arc;

use linefeed::Interface;
use linefeed::terminal::DefaultTerminal;

use log::error;

use crate::chat::component::Component;

pub enum CommandSender {
    Console(Arc<Interface<DefaultTerminal>>)
}

impl CommandSender {
    pub fn send_message(&self, message: Component) {
        match self {
            CommandSender::Console(interface) => match interface.lock_writer_erase() {
                Ok(mut writer) => {
                    if let Err(e) = writeln!(writer, "{}", message) {
                        error!("Failed to send message to console: {}", e);
                    }
                },
                Err(e) => error!("Failed to lock console interface: {}", e)
            }
        }
    }
}
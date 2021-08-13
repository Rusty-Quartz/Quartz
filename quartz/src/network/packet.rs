use std::sync::mpsc::Sender;

use super::AsyncWriteHandle;
use quartz_net::{ClientBoundPacket, PacketBuffer, ServerBoundPacket, WriteToPacket};
use uuid::Uuid;

pub enum WrappedServerBoundPacket {
    External {
        sender: usize,
        packet: ServerBoundPacket,
    },
    ClientConnected {
        id: usize,
        write_handle: AsyncWriteHandle,
    },
    ClientDisconnected {
        id: usize,
    },
    LoginSuccess {
        id: usize,
        uuid: Uuid,
        username: String,
    },
    ConsoleCommand {
        command: String,
    },
    ConsoleCompletion {
        command: String,
        response: Sender<Vec<String>>,
    },
}

impl WrappedServerBoundPacket {
    pub fn external(sender: usize, packet: ServerBoundPacket) -> Self {
        WrappedServerBoundPacket::External { sender, packet }
    }
}

/// A wraper for client-bound packets used internally for sending packets to the connection thread.
pub enum WrappedClientBoundPacket {
    /// A single packet.
    Singleton(ClientBoundPacket),
    /// Multiple packets to be sent all at once.
    Multiple(Box<[Self]>),
    /// A raw byte-buffer.
    Buffer(PacketBuffer),
    /// A generic item which can we written to a packet buffer.
    Custom(Box<dyn WriteToPacket + Send + Sync + 'static>),
    /// Enables compression synchronously on the client channel.
    EnableCompression { threshold: i32 },
    /// Flushes the client channel.
    Flush,
    /// Specifies that the connection should be forcefully terminated.
    Disconnect,
}

impl From<ClientBoundPacket> for WrappedClientBoundPacket {
    fn from(packet: ClientBoundPacket) -> Self {
        WrappedClientBoundPacket::Singleton(packet)
    }
}

impl From<PacketBuffer> for WrappedClientBoundPacket {
    fn from(buffer: PacketBuffer) -> Self {
        WrappedClientBoundPacket::Buffer(buffer)
    }
}

impl From<Box<dyn WriteToPacket + Send + Sync + 'static>> for WrappedClientBoundPacket {
    fn from(packet: Box<dyn WriteToPacket + Send + Sync + 'static>) -> Self {
        WrappedClientBoundPacket::Custom(packet)
    }
}

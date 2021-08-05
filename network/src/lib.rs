pub mod packets {
    use crate::{PacketBuffer, PacketSerdeError};
    include!(concat!(env!("OUT_DIR"), "/packet_def_output.rs"));
}

/// All possible states of a client's connection to the server.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConnectionState {
    /// The handshake state of the connection in which the client selects the next state to enter:
    /// either the `Status` state or `Login` state.
    Handshake,
    /// The client is requesting a server status ping.
    Status,
    /// The client is logging into the server.
    Login,
    /// The client has successfully logged into the server and is playing the game.
    Play,
    /// The client has disconnected.
    Disconnected,
}

/// The numeric protocol version the server uses.
pub const PROTOCOL_VERSION: i32 = 755;
/// The ID for the legacy ping packet.
pub const LEGACY_PING_PACKET_ID: i32 = 0xFE;

mod netutil;
pub mod packet_types;
pub use netutil::*;
mod bitmask;
pub use bitmask::BitMask;

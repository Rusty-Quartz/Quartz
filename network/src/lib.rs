mod bitmask;
mod netutil;
pub mod packet_data;

pub use bitmask::*;
pub use netutil::*;

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

mod build {
    #![allow(clippy::redundant_pattern, clippy::match_single_binding)]
    use super::*;
    include!(concat!(env!("OUT_DIR"), "/packet_def_output.rs"));
}

pub use build::*;

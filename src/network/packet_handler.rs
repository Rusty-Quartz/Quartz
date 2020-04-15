use crate::network::connection::{AsyncClientConnection, ConnectionState};
use crate::util::ioutil::ByteBuffer;

trait AsyncPacketHandler {
//#AsyncPacketHandler

//#end
}

pub trait SyncPacketHandler {
//#SyncPacketHandler

//#end
}

pub enum ServerBoundPacket {
//#ServerBoundPacket

//#end
}

pub enum ClientBoundPacket {
//#ClientBoundPacket

//#end
}

pub fn dispatch_sync_packet(packet: ServerBoundPacket, handler: &mut impl SyncPacketHandler) {
//#dispatch_sync_packet

//#end
}

pub fn serialize(packet: ClientBoundPacket, buffer: &mut ByteBuffer) {
//#serialize

//#end
}

fn handle_packet(conn: &mut AsyncClientConnection, async_handler: &mut DefaultAsyncPacketHandler) {
    let mut buffer = &mut conn.packet_buffer;
    let id = buffer.read_varint();

//#handle_packet

//#end
}

pub async fn handle_async_connection(mut conn: AsyncClientConnection) {
    let mut async_handler = DefaultAsyncPacketHandler::new();

    while conn.connection_state != ConnectionState::Disconnected {
        match conn.read_packet().await {
            Ok(_) => handle_packet(&mut conn, &mut async_handler),
            Err(e) => {
                // TODO: handle properly
                println!("Error in connection handler: {}", e);
                return;
            }
        }
    }

    println!("Client disconnected.");
}

struct DefaultAsyncPacketHandler {

}

impl DefaultAsyncPacketHandler {
    pub fn new() -> DefaultAsyncPacketHandler {
        DefaultAsyncPacketHandler {}
    }
}
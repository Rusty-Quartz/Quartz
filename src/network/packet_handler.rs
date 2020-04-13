use crate::network::connection::{ClientConnection, State};

pub enum Packet {
    // Add packet types here
}

fn handle_packet(conn: &mut ClientConnection) {
    let mut buffer = &mut conn.buffer;

    // Handle the state and packet ID here
}

pub fn handle_connection(mut conn: ClientConnection) {
    while conn.state != State::Disconnected {
        match conn.next_packet() {
            Ok(_) => handle_packet(&mut conn),
            Err(e) => {
                // TODO: handle properly
                println!("Error in connection handler: {}", e);
                return;
            }
        }
    }
}
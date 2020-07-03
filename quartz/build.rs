use serde_json;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    parse_packets();
    println!("cargo:rerun-if-changed=../../assets/Pickaxe/protocol.json");
    println!("cargo:rerun-if-changed=../../assets/Pickaxe/mappings.json");
    println!("cargo:rerun-if-changed=build.rs");
}


fn parse_packets() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("packet_output.rs");

    // Load in json files
    let states_raw: Vec<State> = serde_json::from_str::<Vec<State>>(include_str!("../assets/Pickaxe/protocol.json")).expect("Error reading file");
    let mappings_raw: Mappings = serde_json::from_str::<Mappings>(include_str!("../assets/Pickaxe/mappings.json")).expect("Error reading mappings.json");

    let mut states: Vec<String> = Vec::new();
    let mut server_bound: Vec<Packet> = Vec::new();
    let mut client_bound: Vec<Packet> = Vec::new();

    let mut mappings = HashMap::new();

    // parse mappings
    for type_map in &mappings_raw.types {
        mappings.insert(type_map.name.clone(), type_map.destination.clone());
    }

    // gen packet lists
    for state in states_raw.clone() {
        states.push(state.name);

        if state.server_bound.is_some() {
            for packet in state.server_bound.unwrap() {
                if packet.asyncronous.is_none() || !packet.asyncronous.unwrap() {server_bound.push(packet)};
            }
        }

        if state.client_bound.is_some() {
            for packet in state.client_bound.unwrap() {
                client_bound.push(packet);
            }
        }
    }

    // gen client packet enum
    let mut client_packet_enum = "#[derive(quartz_macros::Listenable)]pub enum ClientBoundPacket {".to_owned();
    client_packet_enum.push_str(&packet_enum_parser(client_bound.clone(), &mappings));
    client_packet_enum.push_str("}");

    // gen server packet enum
    let mut server_packet_enum = "pub enum ServerBoundPacket {".to_owned();
    server_packet_enum.push_str(&packet_enum_parser(server_bound.clone(), &mappings));
    server_packet_enum.push_str("}");

    // gen deserializers
    let mut deserializers = r#"fn handle_packet(conn: &mut AsyncClientConnection, async_handler: &mut AsyncPacketHandler, packet_len: usize) {
                                        let buffer = &mut conn.read_buffer;
                                        let id;
                                        if conn.connection_state == ConnectionState::Handshake && buffer.peek() == LEGACY_PING_PACKET_ID as u8 {
                                            id = LEGACY_PING_PACKET_ID;
                                        } else {
                                            id = buffer.read_varint();
                                        }
                                        match conn.connection_state {"#.to_owned();
    
    for state in states_raw {
        if state.name == "__internal__" {continue;}

        let mut state_str = format!("ConnectionState::{} => {{", state.name);

        if state.server_bound.is_some() {
            state_str.push_str("match id {");

            for packet in state.server_bound.unwrap() {
                let mut packet_str = format!("{} => {{", packet.id);

                for field in &packet.fields {
                    if field.is_used() || field.is_ref() {
                        packet_str.push_str(&format!("let {} = ", field.name));
                    }
                    packet_str.push_str(&format!("buffer.read_{}{};", field.var_type, if field.var_type.contains("(") {""} else {"()"}));
                }

                if packet.is_async() {
                    packet_str.push_str(&format!("async_handler.{}(conn, {});", packet.name.to_ascii_lowercase(), packet.format_params(&mappings_raw)));
                    packet_str.push_str("},");
                } else {
                    packet_str.push_str(&format!("conn.forward_to_server(ServerBoundPacket::{}{}", snake_to_camel(&packet.name), if used_fields(&packet) == 0 {");"} else {"{"}));
                
                    if used_fields(&packet) == 0 {
                        state_str.push_str(&format!("{}}},", packet_str));
                        continue;
                    }

                    for field in &packet.fields {
                        if !field.is_used() {continue;}

                        packet_str.push_str(&format!("{},", field.name))
                    }
                
                    packet_str.push_str("});}, //Stuff");
                }

                state_str.push_str(&packet_str);
            }

            state_str.push_str("_ => invalid_packet!(id, buffer.len())}");
        }

        state_str.push_str("},");

        deserializers.push_str(&state_str);
    }

    deserializers.push_str("_ => {}}}");

    // gen serializers
    let mut serlializers = "pub fn serialize(packet: &ClientBoundPacket, buffer: &mut PacketBuffer) { match packet {".to_owned();

    for packet in client_bound {
        let mut packet_str = format!("ClientBoundPacket::{} {{{}}} => {{", snake_to_camel(&packet.name), packet.struct_params());

        packet_str.push_str(&format!("buffer.write_varint({});", packet.id));

        for field in packet.fields {
            packet_str.push_str(&format!(
                "buffer.write_{}({}{});", 
                field.var_type.to_ascii_lowercase(), 
                if field.var_type == "string" || field.var_type == "byte_array" {""} else {"*"}, 
                field.name))
        }

        packet_str.push_str("},");

        serlializers.push_str(&packet_str);
    }
    serlializers.push_str("}}");

    let invalid_macro = r#"macro_rules! invalid_packet {
    ($id:expr, $len:expr) => {
        warn!("Invalid packet received. ID: {}, Len: {}", $id, $len);
    };
}"#;

    // gen dispatch_sync_packet function
    let mut dispatch = r#"pub fn dispatch_sync_packet(wrapped_packet: &WrappedServerPacket, handler: &mut QuartzServer<'_>) { match &wrapped_packet.packet {"#.to_owned();

    for packet in server_bound {
        dispatch.push_str(&format!("ServerBoundPacket::{} {{{}}} => handler.{}({}),", snake_to_camel(&packet.name), packet.struct_params(), packet.name.to_ascii_lowercase(), if packet.sender_independent.unwrap_or(false) {packet.format_params(&mappings_raw)} else {format!("wrapped_packet.sender, {}", packet.format_params(&mappings_raw))}));
    }

    dispatch.push_str("};}");

    fs::write(&dest_path, format!("{}{}{}{}{}{}", invalid_macro, server_packet_enum, client_packet_enum, deserializers, serlializers, dispatch)).unwrap();
}

fn packet_enum_parser(packet_arr: Vec<Packet>, mappings: &HashMap<String, String>) -> String {
    let mut output = String::new();

    for packet in packet_arr {
        
        // If no fields are used
        if used_fields(&packet) == 0 {
            output.push_str(&format!("{},", snake_to_camel(&packet.name)));
            continue;
        }

        let mut packet_str = format!("{} {{", snake_to_camel(&packet.name));

        for field in packet.fields {
            if field.unused.is_some() && field.unused.unwrap() {continue;}

            packet_str.push_str(&format!("{}: {},", field.name, parse_type(&field.var_type, mappings)))
        }

        packet_str.push_str("},");

        output.push_str(&packet_str);
    }

    output
}

fn snake_to_camel(str: &str) -> String {
    str.replace("_", "")
}

fn used_fields(packet: &Packet) -> usize {
    packet.fields.iter().filter(|field| field.unused.is_none() || !field.unused.unwrap()).collect::<Vec<&Field>>().len()
}

fn parse_type(field: &str, mappings: &HashMap<String, String>) -> String {
    let split = field.split("(").collect::<Vec<&str>>();
    let split = split.get(0).unwrap();
    if mappings.contains_key(split.to_owned()) {
        mappings.get(split.to_owned()).unwrap().to_owned()
    } else {
        split.to_owned().to_owned()
    }
}

#[derive(Deserialize, Clone)]
struct State {
    name: String,
    server_bound: Option<Vec<Packet>>,
    client_bound: Option<Vec<Packet>>
}

#[derive(Deserialize, Clone)]
struct Packet {
    #[serde(rename = "async")]
    asyncronous: Option<bool>,
    unimplemented: Option<bool>,
    sender_independent: Option<bool>,
    name: String,
    id: String,
    fields: Vec<Field>
}

impl Packet {
    pub fn is_async(&self) -> bool {
        self.asyncronous.is_some() && self.asyncronous.unwrap()
    }

    pub fn format_params(&self, mappings: &Mappings) -> String {
        let mut output = String::new();
        if self.fields.iter().filter(|f| f.is_used()).count() == 0 {
            return "".to_owned()
        }
        for field in &self.fields {
            if !field.is_used() {continue;}

            output.push_str(&format!(",{}{}", if !mappings.primitives.contains(&field.var_type) && !field.pass_raw() {"&"} else {""}, field.name))
        }

        output.chars().next().map(|c| &output[c.len_utf8()..]).unwrap().to_owned()
    }
    
    pub fn struct_params(&self) -> String {
        let mut output = String::new();
        for field in &self.fields {
            if !field.is_used() {continue;}
            output.push_str(&format!("{},", field.name))
        }
        output
    }
}

#[derive(Deserialize, Clone)]
struct Field {
    name: String,
    #[serde(rename = "type")]
    var_type: String,
    unused: Option<bool>,
    referenced: Option<bool>,
    pass_raw: Option<bool>
}

impl Field {
    pub fn is_used(&self) -> bool {
        self.unused.is_none() || !self.unused.unwrap()
    }

    pub fn is_ref(&self) -> bool {
        self.referenced.is_some() && self.referenced.unwrap()
    }

    pub fn pass_raw(&self) -> bool {
        self.pass_raw.is_some() && self.pass_raw.unwrap()
    }
}

#[derive(Deserialize)]
struct Mappings {
    types: Vec<TypeMap>,
    primitives: Vec<String>
}

#[derive(Deserialize)]
struct TypeMap {
    name: String,
    #[serde(rename = "type")]
    destination: String
}
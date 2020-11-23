use serde::Deserialize;
use serde_json;
use std::{collections::HashMap, env, fs, path::Path};

const INVALID_MACRO: &str = r#"macro_rules! invalid_packet {
    ($id:expr, $len:expr) => {
        warn!("Invalid packet received. ID: {}, Len: {}", $id, $len);
    };
}"#;

const SERVER_PACKET_START: &str = r#"#[doc = "Packets sent from the client to the server, or packets the server sends internally to itself."]
#[allow(missing_docs)]
pub enum ServerBoundPacket {"#;

const CLIENT_PACKET_START: &str = r#"#[doc = "Packets sent from the server to the client."]
#[allow(missing_docs)]
pub enum ClientBoundPacket {"#;

const DESERIALIZER_START: &str = r#"#[doc = "Deserializes a packet from the connection's buffer and either handles it immediately or forwards it to the server thread."]
async fn handle_packet(conn: &mut AsyncClientConnection, async_handler: &mut AsyncPacketHandler, packet_len: usize) {
    let buffer = &mut conn.read_buffer;
    let id;

    if conn.connection_state == ConnectionState::Handshake && buffer.peek() == LEGACY_PING_PACKET_ID as u8 {
        id = LEGACY_PING_PACKET_ID;
    } else {
        id = buffer.read_varint();
    }

    match conn.connection_state {"#;

const SERIALIZER_START: &str = r#"#[doc = "Serializes a client-bound packet to the given buffer. This function does not apply compression or encryption."]
pub fn serialize(packet: &ClientBoundPacket, buffer: &mut PacketBuffer) {
    match packet {"#;

const DISPATCHER_START: &str = r#"#[doc = "Dispatches a synchronous packet on the server thread."]
pub async fn dispatch_sync_packet<R: Registry>(wrapped_packet: &WrappedServerBoundPacket, handler: &mut QuartzServer<R>) {
    match &wrapped_packet.packet {"#;

pub fn gen_packet_handlers() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("packet_output.rs");

    // Load in json files
    let states_raw: Vec<StatePacketInfo> =
        serde_json::from_str::<Vec<StatePacketInfo>>(include_str!("./assets/protocol.json"))
            .expect("Error reading file");
    let mappings_raw: Mappings =
        serde_json::from_str::<Mappings>(include_str!("./assets/mappings.json"))
            .expect("Error reading mappings.json");

    let mut states: Vec<String> = Vec::new();
    let mut server_bound: Vec<Packet> = Vec::new();
    let mut client_bound: Vec<Packet> = Vec::new();

    let mappings = mappings_raw.types.clone();

    // gen packet lists
    for state in states_raw.clone() {
        states.push(state.name);

        if state.server_bound.is_some() {
            for packet in state.server_bound.unwrap() {
                if !packet.asynchronous.unwrap_or(false) {
                    server_bound.push(packet);
                }
            }
        }

        if state.client_bound.is_some() {
            for packet in state.client_bound.unwrap() {
                client_bound.push(packet);
            }
        }
    }

    let mut server_packet_enum = SERVER_PACKET_START.to_owned();
    server_packet_enum.push_str(&gen_packet_enum(server_bound.clone(), &mappings));

    let mut client_packet_enum = CLIENT_PACKET_START.to_owned();
    client_packet_enum.push_str(&gen_packet_enum(client_bound.clone(), &mappings));

    let deserializers = gen_deserializers(&states_raw, &mappings_raw);

    let serlializers = gen_serializers(&client_bound, &mappings_raw);

    let dispatch = gen_sync_dispatch(&server_bound, &mappings_raw);

    ////////////////////////////////////////
    // Write Output
    ////////////////////////////////////////

    fs::write(
        &dest_path,
        format!(
            "{}\n\n{}\n\n{}\n\n{}\n\n{}\n\n{}",
            INVALID_MACRO,
            server_packet_enum,
            client_packet_enum,
            deserializers,
            serlializers,
            dispatch
        ),
    )
    .unwrap();

    println!("cargo:rerun-if-changed=./assets/Pickaxe/protocol.json");
    println!("cargo:rerun-if-changed=./assets/Pickaxe/mappings.json");
    println!("cargo:rerun-if-changed=buildscript/packets.rs")
}

fn gen_packet_enum(packet_arr: Vec<Packet>, mappings: &HashMap<String, String>) -> String {
    let mut output = String::new();

    for packet in packet_arr {
        // If no fields are used
        if used_field_count(&packet) == 0 {
            output.push_str(&format!("\n\t{},", snake_to_camel(&packet.name)));
            continue;
        }

        let mut packet_str = format!("\n\t{} {{", snake_to_camel(&packet.name));

        for field in packet.fields {
            if field.unused.unwrap_or(false) {
                continue;
            }
            packet_str.push_str(&field.struct_type(mappings));
        }

        packet_str.push_str("\n\t},");
        output.push_str(&packet_str);
    }

    output.push_str("\n}");

    output
}

fn gen_deserializers(states_raw: &Vec<StatePacketInfo>, mappings_raw: &Mappings) -> String {
    let mut deserializers = DESERIALIZER_START.to_owned();

    for state in states_raw {
        if state.name == "__internal__" {
            continue;
        }

        let mut state_str = format!("\n\t\tConnectionState::{} => {{", state.name);

        if state.server_bound.is_some() {
            state_str.push_str("\n\t\t\tmatch id {");

            for packet in state.server_bound.clone().unwrap() {
                let mut packet_str = format!("\n\t\t\t\t{} => {{", packet.id);

                for field in &packet.fields {
                    packet_str.push_str("\n\t\t\t\t\t");
                    packet_str.push_str(&format!("let {} = ", field.name));

                    if field.option {
                        packet_str.push_str(&format!(
                            "if {} {{ Some(buffer.read_{}{}) }} else {{None}};",
                            field.condition,
                            field.var_type,
                            if field.var_type.contains("(") {
                                ""
                            } else {
                                "()"
                            }
                        ))
                    } else {
                        packet_str.push_str(&format!(
                            "buffer.read_{}{};",
                            field.var_type,
                            if field.var_type.contains("(") {
                                ""
                            } else {
                                "()"
                            }
                        ));
                    }
                }

                if packet.is_async() {
                    packet_str.push_str(&format!(
                        "\n\t\t\t\t\tasync_handler.{}(conn, {}).await;",
                        packet.name.to_ascii_lowercase(),
                        packet.format_params(&mappings_raw)
                    ));
                    packet_str.push_str("\n\t\t\t\t},");
                } else {
                    packet_str.push_str(&format!(
                        "\n\t\t\t\t\tconn.forward_to_server(ServerBoundPacket::{}{}",
                        snake_to_camel(&packet.name),
                        if used_field_count(&packet) == 0 {
                            ");"
                        } else {
                            "{"
                        }
                    ));

                    if used_field_count(&packet) == 0 {
                        state_str.push_str(&format!("{}\n\t\t\t\t}},", packet_str));
                        continue;
                    }

                    for field in &packet.fields {
                        if !field.is_used() {
                            continue;
                        }

                        packet_str.push_str(&format!("{},", field.name))
                    }

                    packet_str.push_str("});}, //Stuff");
                }

                state_str.push_str(&packet_str);
            }

            state_str.push_str("\n\t\t\t\t_ => invalid_packet!(id, buffer.len())\n\t\t\t}");
        }

        state_str.push_str("\n\t\t},");
        deserializers.push_str(&state_str);
    }

    deserializers.push_str("\n\t\t_ => {}\n\t}\n}");

    deserializers
}

fn gen_serializers(client_bound: &Vec<Packet>, mappings: &Mappings) -> String {
    let mut serlializers = SERIALIZER_START.to_owned();

    for packet in client_bound {
        let mut packet_str = format!(
            "\n\t\tClientBoundPacket::{} {{{}}} => {{",
            snake_to_camel(&packet.name),
            packet.struct_params()
        );

        packet_str.push_str(&format!("\n\t\t\tbuffer.write_varint({});", packet.id));

        for field in &packet.fields {
            if field.option {
                packet_str.push_str(&format!(
                    "\n\t\t\tmatch {} {{\n\t\t\t\tSome({}) => {{{}}},\n\t\t\t\tNone => \
                     {{}}\n\t\t\t}}",
                    field.name,
                    field.name,
                    if field.array {
                        array_serializer(field, mappings)
                    } else {
                        serializer(field, mappings)
                    }
                ))
            } else {
                packet_str.push_str(&if field.array {
                    array_serializer(field, mappings)
                } else {
                    serializer(field, mappings)
                });
            }
        }

        packet_str.push_str("\n\t\t},");

        serlializers.push_str(&packet_str);
    }

    serlializers.push_str("\n\t}\n}");

    serlializers
}

fn serializer(field: &Field, mappings: &Mappings) -> String {
    format!(
        "\n\t\t\tbuffer.write_{}({}{});",
        field.var_type.to_ascii_lowercase(),
        if !mappings.primitives.contains(&field.var_type) {
            ""
        } else {
            "*"
        },
        field.name
    )
}

fn array_serializer(field: &Field, mappings: &Mappings) -> String {
    format!(
        "\n\t\t\tbuffer.write_{}array::<{}>({}, PacketBuffer::write_{});",
        if mappings
            .primitives
            .contains(&field.var_type.split("(").next().unwrap().to_owned())
        {
            "primative_"
        } else {
            ""
        },
        parse_type(field.var_type.split("(").next().unwrap(), &mappings.types),
        field.name,
        field
            .var_type
            .to_ascii_lowercase()
            .split("(")
            .next()
            .unwrap(),
    )
}

fn gen_sync_dispatch(server_bound: &Vec<Packet>, mappings_raw: &Mappings) -> String {
    let mut dispatch = DISPATCHER_START.to_owned();

    for packet in server_bound.iter().filter(|packet| packet.dispatch) {
        dispatch.push_str(&format!(
            "\n\t\tServerBoundPacket::{} {{{}}} => handler.{}({}).await,",
            snake_to_camel(&packet.name),
            packet.struct_params(),
            packet.name.to_ascii_lowercase(),
            if packet.sender_independent.unwrap_or(false) {
                packet.format_params(mappings_raw)
            } else {
                format!(
                    "wrapped_packet.sender, {}",
                    packet.format_params(mappings_raw)
                )
            }
        ));
    }

    dispatch.push_str("\n\t\t_ => {}\n\t}\n}");

    dispatch
}

fn used_field_count(packet: &Packet) -> usize {
    packet
        .fields
        .iter()
        .filter(|field| !field.unused.unwrap_or(false))
        .count()
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

fn snake_to_camel(str: &str) -> String {
    str.split("_").fold(String::new(), |mut i, s| {
        i.push_str(&(s[.. 1].to_ascii_uppercase() + &s[1 ..].to_owned()));
        i
    })
}

#[derive(Deserialize, Clone)]
struct StatePacketInfo {
    name: String,
    server_bound: Option<Vec<Packet>>,
    client_bound: Option<Vec<Packet>>,
}

#[derive(Deserialize, Clone)]
struct Packet {
    #[serde(rename = "async")]
    asynchronous: Option<bool>,
    unimplemented: Option<bool>,
    sender_independent: Option<bool>,
    #[serde(default = "Packet::dispatch_default")]
    dispatch: bool,
    name: String,
    id: String,
    fields: Vec<Field>,
}

impl Packet {
    pub fn is_async(&self) -> bool {
        self.asynchronous.is_some() && self.asynchronous.unwrap()
    }

    pub fn format_params(&self, mappings: &Mappings) -> String {
        let mut output = String::new();
        if self.fields.iter().filter(|f| f.is_used()).count() == 0 {
            return "".to_owned();
        }
        for field in &self.fields {
            if !field.is_used() {
                continue;
            }

            output.push_str(&format!(
                ",{}{}",
                if !mappings.primitives.contains(&field.var_type) && !field.pass_raw() {
                    "&"
                } else {
                    ""
                },
                field.name
            ))
        }

        output
            .chars()
            .next()
            .map(|c| &output[c.len_utf8() ..])
            .unwrap()
            .to_owned()
    }

    pub fn struct_params(&self) -> String {
        let mut output = String::new();
        for field in &self.fields {
            if !field.is_used() {
                continue;
            }
            output.push_str(&format!("{},", field.name))
        }
        output
    }

    #[inline(always)]
    fn dispatch_default() -> bool {
        true
    }
}

#[derive(Deserialize, Clone)]
struct Field {
    name: String,
    #[serde(rename = "type")]
    var_type: String,
    unused: Option<bool>,
    referenced: Option<bool>,
    pass_raw: Option<bool>,
    #[serde(default)]
    option: bool,
    #[serde(default)]
    array: bool,
    #[serde(default)]
    condition: String,
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

    pub fn struct_type(&self, mappings: &HashMap<String, String>) -> String {
        format!(
            "\n\t\t{}: {}{}{}{}{},",
            self.name,
            if self.option { "Option<" } else { "" },
            if self.array { "Vec<" } else { "" },
            parse_type(&self.var_type, mappings),
            if self.array { ">" } else { "" },
            if self.option { ">" } else { "" }
        )
    }
}

#[derive(Deserialize)]
struct Mappings {
    types: HashMap<String, String>,
    primitives: Vec<String>,
}

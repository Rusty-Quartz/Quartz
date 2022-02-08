use once_cell::unsync::OnceCell;
use proc_macro2::{Literal, TokenStream};
use quartz_macros_impl::packet::{
    gen_deserialize_field,
    ArrayLength,
    Field as CodegenField,
    OptionCondition,
};
use quote::{format_ident, quote};
use serde::Deserialize;
use std::{collections::HashMap, env, ffi::OsStr, fs, path::Path, process::Command};
use syn::Type;

fn format_in_place(file: &OsStr) {
    Command::new("rustfmt")
        .arg(file)
        .output()
        .unwrap_or_else(|_| panic!("Failed to format file: {:?}", file));
}


fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let handler_dest_path = Path::new(&out_dir).join("packet_handler_output.rs");

    // Load in json files
    let states_raw: Vec<StatePacketInfo> =
        serde_json::from_str(include_str!("../assets/protocol.json")).expect("Error reading file");
    let mappings: Mappings = serde_json::from_str(include_str!("../assets/mappings.json"))
        .expect("Error reading mappings.json");

    let mut states: Vec<String> = Vec::new();
    let mut server_bound: Vec<Packet> = Vec::new();
    let mut client_bound: Vec<Packet> = Vec::new();

    // gen packet lists
    for state in states_raw.clone() {
        if state.server_bound.is_some() {
            for packet in state.server_bound.unwrap() {
                server_bound.push(packet);
            }
        }

        if state.client_bound.is_some() {
            for packet in state.client_bound.unwrap() {
                client_bound.push(packet);
            }
        }

        states.push(state.name);
    }

    // We don't have any client bound internal packets
    // let client_packet_enum = gen_packet_enum(
    //     format_ident!("ClientBoundPacket"),
    //     &client_bound,
    //     &states_raw,
    //     false,
    //     &mappings,
    // );

    let handle_packet = gen_handle_packet(&states_raw, &mappings);
    let sync_dispatch = gen_sync_dispatch(&server_bound, &mappings);

    ////////////////////////////////////////
    // Write Output
    ////////////////////////////////////////

    fs::write(
        &handler_dest_path,
        (quote! {
            #handle_packet
            #sync_dispatch
        })
        .to_string(),
    )
    .unwrap();

    format_in_place(handler_dest_path.as_os_str());

    println!("cargo:rerun-if-changed=../../assets/protocol.json");
    println!("cargo:rerun-if-changed=../../assets/mappings.json");
    println!("cargo:rerun-if-changed=buildscript/packets.rs");
}

fn gen_handle_packet(states: &[StatePacketInfo], mappings: &Mappings) -> TokenStream {
    let state_deserializers = states
        .iter()
        .filter(|state_info| state_info.server_bound.is_some() && state_info.name != "__internal__")
        .map(|state_info| {
            let state_name = format_ident!("{}", &state_info.name);
            let match_arms = state_info
                .server_bound
                .as_ref()
                .unwrap()
                .iter()
                .map(|packet| {
                    let id = Literal::i32_unsuffixed(
                        i32::from_str_radix(&packet.id[2..], 16)
                            .expect("Invalid packet ID encountered in JSON.")
                    );
                    let variant_name = format_ident!("{}", snake_to_pascal(&packet.name));
                    let field_names = packet.fields.iter().map(|field| format_ident!("{}", &field.name)).collect::<Vec<_>>();
                    let read_fields = packet
                        .codegen_fields(mappings, false)
                        .map(|(field, _)| gen_deserialize_field(&quote! {quartz_net}, &field, &format_ident!("buffer")));
                    let handler = if packet.asynchronous {
                        let handler_name = format_ident!("handle_{}", packet.name.to_ascii_lowercase());
                        let field_borrows = packet.field_borrows(mappings);
                        quote! { async_handler.#handler_name(conn, #( #field_borrows ),*).await; }
                    } else {
                        quote! { conn.forward_to_server(quartz_net::ServerBoundPacket::#variant_name { #( #field_names ),* }); }
                    };
                    quote! {
                        #id => {
                            #( #read_fields )*
                            #handler
                            Ok(())
                        }
                    }
                });
            quote! {
                quartz_net::ConnectionState::#state_name => {
                    match id {
                        #( #match_arms, )*
                        id @ _ => Err(quartz_net::PacketSerdeError::InvalidId(id))
                    }
                }
            }
        });

    quote! {
        pub(crate) async fn handle_packet(
            conn: &mut AsyncClientConnection,
            async_handler: &mut AsyncPacketHandler,
            packet_len: usize
        ) -> Result<(), quartz_net::PacketSerdeError>
        {
            let initial_len = conn.read_buffer.len();
            let truncated_len = conn.read_buffer.cursor() + packet_len;
            unsafe {
                conn.read_buffer.set_len(truncated_len);
            }

            #[inline(always)]
            async fn handle_packet_internal(
                conn: &mut AsyncClientConnection,
                async_handler: &mut AsyncPacketHandler,
            ) -> Result<(), quartz_net::PacketSerdeError> {
                let buffer = &mut conn.read_buffer;

                let id;
                if conn.connection_state == ConnectionState::Handshake && buffer.peek_one()? == quartz_net::LEGACY_PING_PACKET_ID as u8 {
                    id = quartz_net::LEGACY_PING_PACKET_ID;
                    buffer.read_one()?;
                } else {
                    id = buffer.read_varying::<i32>()?;
                }

                match conn.connection_state {
                    #( #state_deserializers, )*
                    _ => {
                        log::warn!("Attempted to read packet in connection state {:?}", conn.connection_state);
                        Ok(())
                    }
                }
            }
            let mut ret = handle_packet_internal(conn, async_handler).await;

            if conn.read_buffer.len() != truncated_len {
                ret = Err(quartz_net::PacketSerdeError::Internal("Packet buffer written to while being read from"));
            }

            unsafe {
                conn.read_buffer.set_len(initial_len);
            }

            ret
        }
    }
}

fn gen_sync_dispatch(server_bound: &[Packet], mappings: &Mappings) -> TokenStream {
    let match_arms = server_bound
        .iter()
        .filter(|packet| !packet.asynchronous)
        .map(|packet| gen_packet_dispatch(packet, mappings));

    quote! {
        pub async fn dispatch_sync_packet(
            sender: usize,
            packet: &crate::network::ServerBoundPacket,
            handler: &mut crate::QuartzServer
        ) {
            match packet {
                #( #match_arms, )*
                _ => {
                    log::warn!("Async packet sent to sync packet dispatcher");
                }
            }
        }
    }
}

fn gen_packet_dispatch(packet: &Packet, mappings: &Mappings) -> TokenStream {
    let variant_name = format_ident!("{}", snake_to_pascal(&packet.name));
    let field_names = packet
        .fields
        .iter()
        .filter(|field| !field.unused)
        .map(|field| format_ident!("{}", field.name));
    let field_derefs = packet.field_derefs(mappings);
    let handler_name = format_ident!("handle_{}", packet.name.to_ascii_lowercase());
    let sender = if packet.sender_independent {
        None
    } else {
        Some(quote! { sender, })
    };
    quote! {
        quartz_net::ServerBoundPacket::#variant_name { #( #field_names ),* } =>
            handler.#handler_name(#sender #( #field_derefs ),*).await
    }
}

fn parse_type<'a>(field: &'a str, mappings: &'a Mappings) -> &'a str {
    let split = field.split('(').next().unwrap();

    if mappings.types.contains_key(split) {
        mappings.types.get(split).unwrap()
    } else {
        split
    }
}

fn parse_type_meta(field: &'_ str) -> Option<&'_ str> {
    let start = field
        .char_indices()
        .find(|&(_, ch)| ch == '(')
        .map(|(index, _)| index + 1)?;
    Some(&field[start .. field.len() - 1])
}

fn snake_to_pascal(str: &str) -> String {
    str.split('_').fold(String::new(), |mut i, s| {
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
    #[serde(rename = "async", default)]
    asynchronous: bool,
    #[serde(default)]
    sender_independent: bool,
    name: String,
    id: String,
    fields: Vec<Field>,
}

impl Packet {
    pub fn codegen_fields<'a>(
        &'a self,
        mappings: &'a Mappings,
        writing: bool,
    ) -> impl Iterator<Item = (CodegenField, &'a Field)> + 'a {
        self.fields.iter().map(move |field| {
            let name = format_ident!(
                "{}{}",
                if !field.unused || field.referenced || writing {
                    ""
                } else {
                    "_"
                },
                &field.name
            );
            let ty = field.rust_type(mappings).clone();
            let condition = if field.option {
                Some(match &field.condition {
                    Some(expr) =>
                        OptionCondition::Expr(syn::parse_str(expr).unwrap_or_else(|_| {
                            panic!(
                                "Failed to parse condition expression for field {} in packet {}",
                                &field.name, &self.name
                            )
                        })),
                    None => OptionCondition::Prefixed,
                })
            } else {
                None
            };
            let is_option = field.option;
            let varying = mappings.is_varying(&field.var_type);
            if field.array {
                let len = match parse_type_meta(&field.var_type) {
                    Some(meta) => ArrayLength::Expr(syn::parse_str(meta).unwrap_or_else(|_| {
                        panic!(
                            "Failed to parse length expression for array type for field {} in \
                             packet {}",
                            &field.name, &self.name
                        )
                    })),
                    None => ArrayLength::Prefixed,
                };
                (
                    CodegenField::array(
                        name,
                        ty,
                        len,
                        condition,
                        is_option,
                        varying,
                        field.var_type.starts_with("u8"),
                        field.ser_as_nbt,
                    ),
                    field,
                )
            } else {
                (
                    CodegenField::regular(
                        name,
                        ty,
                        condition,
                        is_option,
                        varying,
                        field.ser_as_nbt,
                    ),
                    field,
                )
            }
        })
    }

    pub fn field_borrows<'a>(
        &'a self,
        mappings: &'a Mappings,
    ) -> impl Iterator<Item = TokenStream> + 'a {
        self.fields
            .iter()
            .filter(|field| !field.unused)
            .map(move |field| {
                let field_name = format_ident!("{}", field.name);
                if (field.array || !mappings.primitives.contains(&field.var_type))
                    && !field.pass_raw
                {
                    quote! { &#field_name }
                } else {
                    quote! { #field_name }
                }
            })
    }

    pub fn field_derefs<'a>(
        &'a self,
        mappings: &'a Mappings,
    ) -> impl Iterator<Item = TokenStream> + 'a {
        self.fields
            .iter()
            .filter(|field| !field.unused)
            .map(move |field| {
                let field_name = format_ident!("{}", field.name);
                // This performs a copy, so ignore the pass raw condition
                if mappings.primitives.contains(&field.var_type) {
                    quote! { *#field_name }
                } else {
                    quote! { #field_name }
                }
            })
    }
}

#[derive(Deserialize, Clone)]
struct Field {
    name: String,
    #[serde(rename = "type")]
    var_type: String,
    #[serde(default)]
    unused: bool,
    #[serde(default)]
    referenced: bool,
    #[serde(default)]
    pass_raw: bool,
    #[serde(default)]
    option: bool,
    #[serde(default)]
    array: bool,
    #[serde(default)]
    ser_as_nbt: bool,
    #[serde(default)]
    condition: Option<String>,
    #[serde(skip)]
    cached_type: OnceCell<Type>,
}

impl Field {
    pub fn rust_type(&self, mappings: &Mappings) -> &Type {
        self.cached_type.get_or_init(|| {
            syn::parse_str(&format!(
                "{}{}{}{}{}",
                if self.option { "Option<" } else { "" },
                if self.array { "Box<[" } else { "" },
                parse_type(&self.var_type, mappings),
                if self.array { "]>" } else { "" },
                if self.option { ">" } else { "" }
            ))
            .expect("Invalid type or type mapping encountered in JSON")
        })
    }
}

#[derive(Deserialize)]
struct Mappings {
    types: HashMap<String, String>,
    primitives: Vec<String>,
    variable_repr: Vec<String>,
}

impl Mappings {
    fn is_varying(&self, key: &str) -> bool {
        self.variable_repr
            .iter()
            .any(|varying| key.starts_with(varying))
    }
}

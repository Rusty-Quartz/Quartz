use once_cell::unsync::OnceCell;
use proc_macro2::{Literal, TokenStream};
use quartz_macros_impl::packet::{
    gen_deserialize_field,
    gen_serialize_enum_field,
    Field as CodegenField,
};
use quote::{format_ident, quote};
use serde::Deserialize;
use serde_json;
use std::{collections::HashMap, env, fs, path::Path};
use syn::Type;

pub fn gen_packet_handlers() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let packet_enums_dest_path = Path::new(&out_dir).join("packet_def_output.rs");
    let handler_dest_path = Path::new(&out_dir).join("packet_handler_output.rs");

    // Load in json files
    let states_raw: Vec<StatePacketInfo> =
        serde_json::from_str(include_str!("./assets/protocol.json")).expect("Error reading file");
    let mappings: Mappings = serde_json::from_str(include_str!("./assets/mappings.json"))
        .expect("Error reading mappings.json");

    let mut states: Vec<String> = Vec::new();
    let mut server_bound: Vec<Packet> = Vec::new();
    let mut client_bound: Vec<Packet> = Vec::new();

    // gen packet lists
    for state in states_raw.clone() {
        states.push(state.name);

        if state.server_bound.is_some() {
            for packet in state.server_bound.unwrap() {
                if !packet.asynchronous {
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

    let client_packet_enum = gen_client_packet_enum(&client_bound, &mappings);
    let server_packet_enum = gen_server_packet_enum(&states_raw, &mappings);
    let handle_packet = gen_handle_packet(&states_raw, &mappings);
    let sync_dispatch = gen_sync_dispatch(&server_bound, &mappings);

    ////////////////////////////////////////
    // Write Output
    ////////////////////////////////////////

    fs::write(
        &packet_enums_dest_path,
        (quote! {
            #client_packet_enum
            #server_packet_enum
        })
        .to_string(),
    )
    .unwrap();
    fs::write(
        &handler_dest_path,
        (quote! {
            #handle_packet
            #sync_dispatch
        })
        .to_string(),
    )
    .unwrap();

    super::format_in_place(packet_enums_dest_path.as_os_str());
    super::format_in_place(handler_dest_path.as_os_str());

    println!("cargo:rerun-if-changed=./assets/Pickaxe/protocol.json");
    println!("cargo:rerun-if-changed=./assets/Pickaxe/mappings.json");
    println!("cargo:rerun-if-changed=buildscript/packets.rs")
}

fn gen_client_packet_enum(packet_arr: &[Packet], mappings: &Mappings) -> TokenStream {
    let variant_names = packet_arr
        .iter()
        .map(|packet| format_ident!("{}", snake_to_pascal(&packet.name)))
        .collect::<Vec<_>>();
    let variants = packet_arr.iter().enumerate().map(|(index, packet)| {
        let variant_name = &variant_names[index];
        if packet.fields.is_empty() {
            quote! { #variant_name }
        } else {
            let fields = packet
                .fields
                .iter()
                .map(|field| field.struct_field_def(mappings));
            quote! {
                #variant_name { #( #fields ),* }
            }
        }
    });
    let write_variants = packet_arr.iter().enumerate().map(|(index, packet)| {
        let variant_name = &variant_names[index];
        let id = Literal::i32_unsuffixed(
            i32::from_str_radix(&packet.id[2 ..], 16)
                .expect("Invalid packet ID encountered in JSON."),
        );
        let field_names = packet
            .fields
            .iter()
            .map(|field| format_ident!("{}", &field.name))
            .collect::<Vec<_>>();
        let unpack_fields = if packet.fields.is_empty() {
            None
        } else {
            Some(quote! { { #( #field_names ),* } })
        };
        let write_fields = packet
            .codegen_fields(mappings)
            .map(|field| gen_serialize_enum_field(&field, &format_ident!("buffer")));
        quote! {
            Self::#variant_name #unpack_fields => {
                buffer.write_varying(&#id);
                #( #write_fields )*
            }
        }
    });

    quote! {
        pub enum ClientBoundPacket {
            #( #variants ),*
        }

        impl crate::network::WriteToPacket for ClientBoundPacket {
            fn write_to(&self, buffer: &mut crate::network::PacketBuffer) {
                match self {
                    #( #write_variants ),*
                }
            }
        }
    }
}

fn gen_server_packet_enum(states: &[StatePacketInfo], mappings: &Mappings) -> TokenStream {
    let variants = states
        .iter()
        .flat_map(|state_info| state_info.server_bound.as_ref().into_iter().flatten())
        .map(|packet| {
            let variant_name = format_ident!("{}", snake_to_pascal(&packet.name));
            if packet.fields.is_empty() {
                quote! { #variant_name }
            } else {
                let fields = packet
                    .fields
                    .iter()
                    .map(|field| field.struct_field_def(mappings));
                quote! {
                    #variant_name { #( #fields ),* }
                }
            }
        });

    quote! {
        pub enum ServerBoundPacket {
            #( #variants ),*
        }
    }
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
                        .codegen_fields(mappings)
                        .map(|field| gen_deserialize_field(&field, &format_ident!("buffer")));
                    let handler = if packet.asynchronous {
                        let handler_name = format_ident!("handle_{}", packet.name.to_ascii_lowercase());
                        let field_borrows = packet.field_borrows(mappings);
                        quote! { async_handler.#handler_name(conn, #( #field_borrows ),*).await; }
                    } else {
                        quote! { conn.forward_to_server(crate::network::packet::ServerBoundPacket::#variant_name { #( #field_names ),* }); }
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
                crate::network::ConnectionState::#state_name => {
                    match id {
                        #( #match_arms, )*
                        id @ _ => Err(crate::network::PacketSerdeError::InvalidId(id))
                    }
                }
            }
        });

    quote! {
        pub(crate) async fn handle_packet(
            conn: &mut AsyncClientConnection,
            async_handler: &mut AsyncPacketHandler,
            packet_len: usize
        ) -> Result<(), crate::network::PacketSerdeError>
        {
            let initial_len = conn.read_buffer.len();
            let truncated_len = conn.read_buffer.cursor() + packet_len;
            unsafe {
                conn.read_buffer.set_len(truncated_len);
            }

            async fn handle_packet_internal(
                conn: &mut AsyncClientConnection,
                async_handler: &mut AsyncPacketHandler,
            ) -> Result<(), crate::network::PacketSerdeError> {
                let buffer = &mut conn.read_buffer;

                let id;
                if conn.connection_state == ConnectionState::Handshake && buffer.peek()? == LEGACY_PING_PACKET_ID as u8 {
                    id = LEGACY_PING_PACKET_ID;
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
                ret = Err(crate::network::PacketSerdeError::Internal("Packet buffer written to while being read from"));
            }

            unsafe {
                conn.read_buffer.set_len(initial_len);
            }

            ret
        }
    }
}

fn gen_sync_dispatch(server_bound: &[Packet], mappings: &Mappings) -> TokenStream {
    let match_arms = server_bound.iter().map(|packet| {
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
            crate::network::packet::ServerBoundPacket::#variant_name { #( #field_names ),* } =>
                handler.#handler_name(#sender #( #field_derefs ),*).await
        }
    });

    quote! {
        pub async fn dispatch_sync_packet(wrapped_packet: &crate::network::packet::WrappedServerBoundPacket, handler: &mut crate::QuartzServer) {
            let sender = wrapped_packet.sender;
            match &wrapped_packet.packet {
                #( #match_arms, )*
                _ => {
                    log::warn!("Async packet sent to sync packet dispatcher");
                }
            }
        }
    }
}

fn parse_type<'a>(field: &'a str, mappings: &'a Mappings) -> &'a str {
    let split = field.split("(").next().unwrap();

    if mappings.types.contains_key(split) {
        mappings.types.get(split).unwrap()
    } else {
        split
    }
}

fn parse_type_meta<'a>(field: &'a str) -> Option<&'a str> {
    let start = field
        .char_indices()
        .find(|&(_, ch)| ch == '(')
        .map(|(index, _)| index + 1)?;
    Some(&field[start .. field.len() - 1])
}

fn snake_to_pascal(str: &str) -> String {
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
    #[serde(rename = "async", default)]
    asynchronous: bool,
    #[serde(default)]
    unimplemented: bool,
    #[serde(default)]
    sender_independent: bool,
    #[serde(default = "Packet::dispatch_default")]
    dispatch: bool,
    name: String,
    id: String,
    fields: Vec<Field>,
}

impl Packet {
    pub fn codegen_fields<'a>(
        &'a self,
        mappings: &'a Mappings,
    ) -> impl Iterator<Item = CodegenField> + 'a {
        self.fields.iter().map(move |field| {
            let name = format_ident!(
                "{}{}",
                if !field.unused || field.referenced {
                    ""
                } else {
                    "_"
                },
                &field.name
            );
            let ty = field.rust_type(mappings).clone();
            let condition = if field.option {
                Some(syn::parse_str(&field.condition).expect(&format!(
                    "Failed to parse condition expression for field {} in packet {}",
                    &field.name, &self.name
                )))
            } else {
                None
            };
            let is_option = field.option;
            let varying = mappings.is_varying(&field.var_type);
            if field.array {
                let len = syn::parse_str(parse_type_meta(&field.var_type).expect(&format!(
                    "Expected length metadata for array type for field {} in packet {}",
                    &field.name, &self.name
                )))
                .expect(&format!(
                    "Failed to parse length expression for array type for field {} in packet {}",
                    &field.name, &self.name
                ));
                CodegenField::array(
                    name,
                    ty,
                    len,
                    condition,
                    is_option,
                    varying,
                    field.var_type.starts_with("u8"),
                )
            } else {
                CodegenField::regular(name, ty, condition, is_option, varying)
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
                if !mappings.primitives.contains(&field.var_type) && !field.pass_raw {
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
    condition: String,
    #[serde(skip)]
    cached_type: OnceCell<Type>,
}

impl Field {
    pub fn rust_type(&self, mappings: &Mappings) -> &Type {
        self.cached_type.get_or_init(|| {
            syn::parse_str(&format!(
                "{}{}{}{}{}",
                if self.option { "Option<" } else { "" },
                if self.array { "Vec<" } else { "" },
                parse_type(&self.var_type, mappings),
                if self.option { ">" } else { "" },
                if self.array { ">" } else { "" }
            ))
            .expect("Invalid type or type mapping encountered in JSON")
        })
    }

    pub fn struct_field_def(&self, mappings: &Mappings) -> TokenStream {
        let name = format_ident!("{}", self.name);
        let ty: Type = match syn::parse_str(parse_type(&self.var_type, mappings)) {
            Ok(ty) => ty,
            Err(e) => return e.to_compile_error(),
        };
        match (self.option, self.array) {
            (true, true) => quote! {
                #name: Option<Vec<#ty>>
            },
            (true, false) => quote! {
                #name: Option<#ty>
            },
            (false, true) => quote! {
                #name: Vec<#ty>
            },
            (false, false) => quote! {
                #name: #ty
            },
        }
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

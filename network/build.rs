use once_cell::unsync::OnceCell;
use proc_macro2::{Literal, TokenStream};
use quartz_macros_impl::packet::{
    gen_deserialize_field,
    gen_serialize_enum_field,
    ArrayLength,
    Field as CodegenField,
    OptionCondition,
};
use quote::{format_ident, quote};
use serde::Deserialize;
use std::{collections::HashMap, env, ffi::OsStr, fs, path::Path, process::Command};
use syn::{Ident, Type};

fn format_in_place(file: &OsStr) {
    Command::new("rustfmt")
        .arg(file)
        .output()
        .unwrap_or_else(|_| panic!("Failed to format file: {:?}", file));
}


fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let packet_enums_dest_path = Path::new(&out_dir).join("packet_def_output.rs");
    let handler_dest_path = Path::new(&out_dir).join("packet_handler_output.rs");

    // Load in json files
    let states_raw: Vec<StatePacketInfo> = serde_json::from_str(
        &include_str!("../assets/protocol.json").replace("quartz_net", "crate"),
    )
    .expect("Error reading file");
    let mappings: Mappings = serde_json::from_str(
        &include_str!("../assets/mappings.json").replace("quartz_net", "crate"),
    )
    .expect("Error reading mappings.json");

    let mut states: Vec<String> = Vec::new();
    let mut server_bound: Vec<Packet> = Vec::new();
    let mut client_bound: Vec<Packet> = Vec::new();

    // gen packet lists
    for state in states_raw.clone() {
        if state.server_bound.is_some() && state.name != "__internal__" {
            for packet in state.server_bound.unwrap() {
                server_bound.push(packet);
            }
        }

        if state.client_bound.is_some() && state.name != "__internal__" {
            for packet in state.client_bound.unwrap() {
                client_bound.push(packet);
            }
        }

        states.push(state.name);
    }

    let client_packet_enum = gen_packet_enum(
        format_ident!("ClientBoundPacket"),
        &client_bound,
        &states_raw,
        false,
        &mappings,
    );
    let server_packet_enum = gen_packet_enum(
        format_ident!("ServerBoundPacket"),
        &server_bound,
        &states_raw,
        true,
        &mappings,
    );

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

    format_in_place(packet_enums_dest_path.as_os_str());
    format_in_place(handler_dest_path.as_os_str());

    println!("cargo:rerun-if-changed=../../assets/protocol.json");
    println!("cargo:rerun-if-changed=../../assets/mappings.json");
    println!("cargo:rerun-if-changed=buildscript/packets.rs");
}

fn gen_packet_enum(
    enum_name: Ident,
    packet_arr: &[Packet],
    states: &[StatePacketInfo],
    is_server_bound: bool,
    mappings: &Mappings,
) -> TokenStream {
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
    let any_internal = packet_arr.iter().any(|packet| packet.internal);
    let write_variants = packet_arr
        .iter()
        .enumerate()
        .filter(|(_, packet)| !packet.internal)
        .map(|(index, packet)| {
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
                .codegen_fields(mappings, true)
                .map(|(field, _)| gen_serialize_enum_field(&field, &format_ident!("buffer")));
            quote! {
                Self::#variant_name #unpack_fields => {
                    buffer.write_varying(&#id);
                    #( #write_fields )*
                }
            }
        });
    let state_deserializers = states
        .iter()
        .filter(|state_info| {
            if state_info.name == "__internal__" {
                return false;
            }
            if is_server_bound {
                state_info.server_bound.is_some()
            } else {
                state_info.client_bound.is_some()
            }
        })
        .map(|state_info| {
            let state_name = format_ident!("{}", &state_info.name);
            let packets = if is_server_bound {
                state_info.server_bound.as_ref().unwrap()
            } else {
                state_info.client_bound.as_ref().unwrap()
            };
            let match_arms = packets.iter().map(|packet| {
                let id = Literal::i32_unsuffixed(
                    i32::from_str_radix(&packet.id[2 ..], 16)
                        .expect("Invalid packet ID encountered in JSON."),
                );
                let variant_name = format_ident!("{}", snake_to_pascal(&packet.name));
                let field_names = packet
                    .fields
                    .iter()
                    .map(|field| format_ident!("{}", &field.name))
                    .collect::<Vec<_>>();
                let read_fields = packet.codegen_fields(mappings, true).map(
                    |(codegen_field, field)| match &field.deserialize_with {
                        Some(deserializer) => {
                            let name = &codegen_field.name;
                            let deserializer: TokenStream = syn::parse_str(deserializer).unwrap();
                            quote! {
                                let #name = #deserializer;
                            }
                        }
                        None => gen_deserialize_field(
                            &quote! {crate},
                            &codegen_field,
                            &format_ident!("buffer"),
                        ),
                    },
                );
                quote! {
                    #id => {
                        #( #read_fields )*
                        Ok(#enum_name::#variant_name { #( #field_names ),* })
                    }
                }
            });

            quote! {
                crate::ConnectionState::#state_name => {
                    match id {
                        #( #match_arms, )*
                        id @ _ => Err(crate::PacketSerdeError::InvalidId(id))
                    }
                }
            }
        });

    let default_case = if any_internal {
        Some(quote! { _ => unimplemented!("WriteToPacket unimplemented for {:?}", self) })
    } else {
        None
    };

    quote! {
        #[derive(Debug)]
        pub enum #enum_name {
            #( #variants ),*
        }

        impl #enum_name {
            pub fn read_from(
                buffer: &mut PacketBuffer,
                connection_state: crate::ConnectionState,
                packet_len: usize
            ) -> Result<Self, crate::PacketSerdeError>
            {
                let initial_len = buffer.len();
                let truncated_len = buffer.cursor() + packet_len;

                if truncated_len > initial_len {
                    return Err(PacketSerdeError::EndOfBuffer);
                }

                unsafe {
                    buffer.set_len(truncated_len);
                }

                #[inline(always)]
                fn read_internal(
                    buffer: &mut PacketBuffer,
                    connection_state: crate::ConnectionState,
                ) -> Result<#enum_name, crate::PacketSerdeError> {
                    let id;
                    if connection_state == crate::ConnectionState::Handshake && buffer.peek_one()? == crate::LEGACY_PING_PACKET_ID as u8 {
                        id = crate::LEGACY_PING_PACKET_ID;
                        buffer.read_one()?;
                    } else {
                        id = buffer.read_varying::<i32>()?;
                    }

                    match connection_state {
                        #( #state_deserializers, )*
                        _ => Err(crate::PacketSerdeError::Internal("Attempted to read packet in invalid connection state"))
                    }
                }
                let mut ret = read_internal(buffer, connection_state);

                if buffer.len() != truncated_len {
                    ret = Err(crate::PacketSerdeError::Internal("Packet buffer written to while being read from"));
                }

                unsafe {
                    buffer.set_len(initial_len);
                }

                ret
            }
        }

        impl crate::WriteToPacket for #enum_name {
            fn write_to(&self, buffer: &mut crate::PacketBuffer) {
                match self {
                    #( #write_variants ),*
                    #default_case
                }
            }
        }
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
    #[serde(default)]
    internal: bool,
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
    option: bool,
    #[serde(default)]
    array: bool,
    #[serde(default)]
    ser_as_nbt: bool,
    #[serde(default)]
    condition: Option<String>,
    #[serde(default)]
    deserialize_with: Option<String>,
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

    pub fn struct_field_def(&self, mappings: &Mappings) -> TokenStream {
        let name = format_ident!("{}", self.name);
        let ty: Type = match syn::parse_str(parse_type(&self.var_type, mappings)) {
            Ok(ty) => ty,
            Err(e) => return e.to_compile_error(),
        };
        match (self.option, self.array) {
            (true, true) => quote! {
                #name: Option<Box<[#ty]>>
            },
            (true, false) => quote! {
                #name: Option<#ty>
            },
            (false, true) => quote! {
                #name: Box<[#ty]>
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
    #[allow(dead_code)]
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

mod gen;
mod parse;

pub use gen::*;
pub use parse::*;

use proc_macro2::TokenStream;
use syn::{DeriveInput, Data};

pub fn derive_write_to_packet_impl(input: DeriveInput) -> TokenStream {
    match &input.data {
        Data::Enum(data) => {
            let variants = match parse_enum(data, Side::Write) {
                Ok(variants) => variants,
                Err(e) => return e.to_compile_error().into()
            };
            gen_enum_serializer_impl(input, &variants).into()
        },
        _ => {
            let fields = match parse_fields(&input, Side::Write) {
                Ok(fields) => fields,
                Err(e) => return e.to_compile_error().into(),
            };
            gen_struct_serializer_impl(input, &fields).into()
        }
    }
}

pub fn derive_read_from_packet_impl(input: DeriveInput) -> TokenStream {
    let fields = match parse_fields(&input, Side::Read) {
        Ok(fields) => fields,
        Err(e) => return e.to_compile_error().into(),
    };
    gen_struct_deserializer_impl(input, &fields).into()
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    Read,
    Write
}
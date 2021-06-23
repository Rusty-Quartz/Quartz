extern crate proc_macro;

mod gen;
mod parse;

use gen::*;
use parse::parse_fields;
use proc_macro2::{Span, TokenStream};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Error, Ident};

#[proc_macro_derive(WriteToPacket, attributes(packet_serde))]
pub fn derive_write_to_packet(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let fields = match parse_fields(&input) {
        Ok(fields) => fields,
        Err(e) => return e.to_compile_error().into(),
    };
    gen_serializer(input, &fields).into()
}

#[proc_macro_derive(ReadFromPacket, attributes(packet_serde))]
pub fn derive_read_from_packet(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let fields = match parse_fields(&input) {
        Ok(fields) => fields,
        Err(e) => return e.to_compile_error().into(),
    };
    gen_deserializer(input, &fields).into()
}

pub(crate) fn the_crate() -> TokenStream {
    match crate_name("quartz") {
        Ok(FoundCrate::Itself) => quote! { crate },
        Ok(FoundCrate::Name(name)) => {
            let name = Ident::new(&name, Span::call_site());
            quote! { ::#name }
        }
        Err(e) => Error::new(Span::call_site(), format!("{}", e)).to_compile_error(),
    }
}

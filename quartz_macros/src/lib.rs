extern crate proc_macro;

use quartz_macros_impl::*;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(WriteToPacket, attributes(packet_serde))]
pub fn derive_write_to_packet(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    packet::derive_write_to_packet_impl(parse_macro_input!(item as DeriveInput)).into()
}

#[proc_macro_derive(ReadFromPacket, attributes(packet_serde))]
pub fn derive_read_from_packet(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    packet::derive_read_from_packet_impl(parse_macro_input!(item as DeriveInput)).into()
}

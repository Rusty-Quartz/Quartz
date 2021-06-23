use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{DeriveInput, Ident};

use crate::{
    parse::{Field, FieldType},
    the_crate,
};

pub fn gen_serializer(input: DeriveInput, fields: &[Field]) -> TokenStream {
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let name = &input.ident;
    let serialize_fields = fields.iter().map(serialize_field);
    let the_crate = the_crate();

    quote! {
        impl #impl_generics #the_crate::network::WriteToPacket for #name #ty_generics #where_clause {
            fn write_to(&self, __buffer: &mut #the_crate::network::PacketBuffer) {
                #( #serialize_fields )*
            }
        }
    }
}

pub fn gen_deserializer(input: DeriveInput, fields: &[Field]) -> TokenStream {
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let name = &input.ident;
    let deserialize_fields = fields.iter().map(deserialize_field);
    let field_names = fields.iter().map(|field| &field.name);
    let the_crate = the_crate();

    quote! {
        impl #impl_generics #the_crate::network::ReadFromPacket for #name #ty_generics #where_clause {
            fn read_from(__buffer: &mut #the_crate::network::PacketBuffer) -> Self {
                #( #deserialize_fields )*
                Self { #( #field_names ),* }
            }
        }
    }
}

fn serialize_field(field: &Field) -> TokenStream {
    let name = &field.name;
    let write_fn = match &field.ty {
        FieldType::Regular(_) => Ident::new(
            if field.varying {
                "write_varying"
            } else {
                "write"
            },
            Span::call_site(),
        ),
        FieldType::Array { .. } => Ident::new(
            if field.varying {
                "write_array_varying"
            } else {
                "write"
            },
            Span::call_site(),
        ),
    };

    if field.is_option {
        quote! {
            if let ::core::option::Option::Some(__value) = &self.#name {
                __buffer.#write_fn(__value);
            }
        }
    } else {
        quote! { __buffer.#write_fn(&self.#name); }
    }
}

fn deserialize_field(field: &Field) -> TokenStream {
    let name = &field.name;
    let read_impl = match &field.ty {
        FieldType::Regular(_) =>
            if field.varying {
                quote! { __buffer.read_varying() }
            } else {
                quote! { __buffer.read() }
            },
        FieldType::Array { len, .. } =>
            if field.varying {
                quote! { __buffer.read_array_varying(#len) }
            } else {
                quote! { __buffer.read_array(#len) }
            },
    };
    let read_impl = if field.is_option {
        quote! { ::core::option::Option::Some(#read_impl) }
    } else {
        read_impl
    };

    match &field.condition {
        Some(condition) => quote! {
            let #name = if #condition {
                #read_impl
            } else {
                ::core::default::Default::default()
            };
        },
        None => quote! {
            let #name = #read_impl;
        },
    }
}

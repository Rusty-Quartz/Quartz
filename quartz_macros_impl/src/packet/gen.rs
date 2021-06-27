use proc_macro2::{Span, TokenStream};
use quote::{quote, format_ident};
use syn::{DeriveInput, Ident};

use crate::the_crate;
use super::parse::{EnumStructVariant, Field, FieldType};

pub fn gen_struct_serializer_impl(input: DeriveInput, fields: &[Field]) -> TokenStream {
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let name = &input.ident;
    let serialize_fields = fields.iter().map(|field| gen_serialize_struct_field(field, &format_ident!("__buffer")));
    let the_crate = the_crate();

    quote! {
        impl #impl_generics #the_crate::network::WriteToPacket for #name #ty_generics #where_clause {
            fn write_to(&self, __buffer: &mut #the_crate::network::PacketBuffer) {
                #( #serialize_fields )*
            }
        }
    }
}

pub fn gen_enum_serializer_impl(input: DeriveInput, variants: &[EnumStructVariant]) -> TokenStream {
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let name = &input.ident;
    let any_unit = variants.iter().any(|variant| variant.fields.is_empty());
    let the_crate = the_crate();

    let serialize_variants = variants
        .iter()
        .filter(|variant| !variant.fields.is_empty())
        .map(|variant| {
            let var_name = &variant.name;
            let field_names = variant
                .fields
                .iter()
                .map(|field| &field.name);
            let serialize_fields = variant
                .fields
                .iter()
                .map(|field| gen_serialize_enum_field(field, &format_ident!("__buffer")));
            let field_unpacking = if variant.is_tuple {
                quote! { ( #( #field_names ),* ) }
            } else {
                quote! { { #( #field_names ),* } }
            };
            quote! {
                Self::#var_name #field_unpacking => { #( #serialize_fields )* }
            }
        });
    let default_branch = if any_unit {
        Some(quote!{ _ => {} })
    } else {
        None
    };

    quote! {
        impl #impl_generics #the_crate::network::WriteToPacket for #name #ty_generics #where_clause {
            fn write_to(&self, __buffer: &mut #the_crate::network::PacketBuffer) {
                match self {
                    #( #serialize_variants, )*
                    #default_branch
                }
            }
        }
    }
}

pub fn gen_struct_deserializer_impl(input: DeriveInput, fields: &[Field]) -> TokenStream {
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let name = &input.ident;
    let deserialize_fields = fields.iter().map(|field| gen_deserialize_field(field, &format_ident!("__buffer")));
    let field_names = fields.iter().map(|field| &field.name);
    let the_crate = the_crate();

    quote! {
        impl #impl_generics #the_crate::network::ReadFromPacket for #name #ty_generics #where_clause {
            fn read_from(__buffer: &mut #the_crate::network::PacketBuffer) -> ::core::result::Result<Self, #the_crate::network::PacketSerdeError> {
                #( #deserialize_fields )*
                Ok(Self { #( #field_names ),* })
            }
        }
    }
}

pub fn gen_serialize_struct_field(field: &Field, buffer_ident: &Ident) -> TokenStream {
    let name = &field.name;
    let write_fn = write_fn_for_field(field);

    if field.is_option {
        quote! {
            if let ::core::option::Option::Some(__value) = &self.#name {
                #buffer_ident.#write_fn(__value);
            }
        }
    } else {
        quote! { #buffer_ident.#write_fn(&self.#name); }
    }
}

pub fn gen_serialize_enum_field(field: &Field, buffer_ident: &Ident) -> TokenStream {
    let name = &field.name;
    let write_fn = write_fn_for_field(field);

    if field.is_option {
        quote! {
            if let ::core::option::Option::Some(__value) = #name {
                #buffer_ident.#write_fn(__value);
            }
        }
    } else {
        quote! { #buffer_ident.#write_fn(#name); }
    }
}

fn write_fn_for_field(field: &Field) -> Ident {
    match &field.ty {
        FieldType::Regular => Ident::new(
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
                "write_array"
            },
            Span::call_site(),
        ),
    }
}

pub fn gen_deserialize_field(field: &Field, buffer_ident: &Ident) -> TokenStream {
    let name = &field.name;
    let ty = &field.raw_ty;
    let read_impl = match &field.ty {
        FieldType::Regular =>
            if field.varying {
                quote! { #buffer_ident.read_varying()? }
            } else {
                quote! { #buffer_ident.read()? }
            },
        FieldType::Array { len } =>
            if field.varying {
                quote! { #buffer_ident.read_array_varying(#len)? }
            } else {
                quote! { #buffer_ident.read_array(#len)? }
            },
    };
    let read_impl = if field.is_option {
        quote! { ::core::option::Option::Some(#read_impl) }
    } else {
        read_impl
    };

    match &field.condition {
        Some(condition) => quote! {
            let #name: #ty = if #condition {
                #read_impl
            } else {
                ::core::default::Default::default()
            };
        },
        None => quote! {
            let #name: #ty = #read_impl;
        },
    }
}

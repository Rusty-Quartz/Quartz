use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{DeriveInput, Ident};

use super::{OptionCondition, parse::{EnumStructVariant, Field, FieldType, ArrayLength}};
use crate::the_crate;

pub fn gen_struct_serializer_impl(input: DeriveInput, fields: &[Field]) -> TokenStream {
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let name = &input.ident;
    let serialize_fields = fields
        .iter()
        .map(|field| gen_serialize_struct_field(field, &format_ident!("__buffer")));
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
            let field_names = variant.fields.iter().map(|field| &field.name);
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
        Some(quote! { _ => {} })
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
    let deserialize_fields = fields
        .iter()
        .map(|field| gen_deserialize_field(field, &format_ident!("__buffer")));
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
        let field_ref = quote! { __value };
        let write_condition = field.condition
            .as_ref()
            .map(|condition| condition.gen_write_condition(&quote! { self.#name }, buffer_ident))
            .flatten();
        let len_prefix = field.opt_write_length_prefix(&field_ref, buffer_ident);
        quote! {
            #write_condition
            if let ::core::option::Option::Some(#field_ref) = &self.#name {
                #len_prefix
                #buffer_ident.#write_fn(#field_ref);
            }
        }
    } else {
        let field_ref = quote! { self.#name };
        let len_prefix = field.opt_write_length_prefix(&field_ref, buffer_ident);
        quote! {
            #len_prefix
            #buffer_ident.#write_fn(&#field_ref);
        }
    }
}

pub fn gen_serialize_enum_field(field: &Field, buffer_ident: &Ident) -> TokenStream {
    let name = &field.name;
    let write_fn = write_fn_for_field(field);

    if field.is_option {
        let field_ref = quote! { __value };
        let write_condition = field.condition
            .as_ref()
            .map(|condition| condition.gen_write_condition(&quote! { #name }, buffer_ident))
            .flatten();
        let len_prefix = field.opt_write_length_prefix(&field_ref, buffer_ident);
        
        quote! {
            #write_condition
            if let ::core::option::Option::Some(#field_ref) = #name {
                #len_prefix
                #buffer_ident.#write_fn(#field_ref);
            }
        }
    } else {
        let field_ref = quote! { #name };
        let len_prefix = field.opt_write_length_prefix(&field_ref, buffer_ident);
        quote! {
            #len_prefix
            #buffer_ident.#write_fn(#field_ref);
        }
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
        FieldType::Array { len } => {
            let len = len.gen_read_length(buffer_ident);
            if field.varying {
                quote! {{
                    #len
                    #buffer_ident.read_array_varying(__len)?
                }}
            } else {
                quote! {{
                    #len
                    #buffer_ident.read_array(__len)?
                }}
            }
        }
    };
    let read_impl = if field.is_option {
        quote! { ::core::option::Option::Some(#read_impl) }
    } else {
        read_impl
    };

    match &field.condition {
        Some(condition) => {
            let condition = condition.gen_read_condition(buffer_ident);
            quote! {
                let #name: #ty = if #condition {
                    #read_impl
                } else {
                    ::core::default::Default::default()
                };
            }
        },
        None => quote! {
            let #name: #ty = #read_impl;
        },
    }
}

impl Field {
    fn opt_write_length_prefix(&self, field_ref: &TokenStream, buffer_ident: &Ident) -> Option<TokenStream> {
        match &self.ty {
            FieldType::Array { len } => len.gen_write_length(field_ref, buffer_ident),
            _ => None
        }
    }
}

impl ArrayLength {
    fn gen_read_length(&self, buffer_ident: &Ident) -> TokenStream {
        match self {
            ArrayLength::Expr(expr) => quote! { let __len = #expr; },
            ArrayLength::Prefixed => quote! { let __len = #buffer_ident.read_varying::<i32>()? as usize; },
            ArrayLength::Greedy => quote! { let __len = #buffer_ident.remaining(); }
        }
    }

    fn gen_write_length(&self, field_ref: &TokenStream, buffer_ident: &Ident) -> Option<TokenStream> {
        match self {
            // The field would have already been written
            ArrayLength::Expr(_) => None,
            ArrayLength::Prefixed => Some(quote! { #buffer_ident.write_varying(&(#field_ref.len() as i32)); }),
            // Length inferred
            ArrayLength::Greedy => None
        }
    }
}

impl OptionCondition {
    fn gen_read_condition(&self, buffer_ident: &Ident) -> TokenStream {
        match self {
            OptionCondition::Expr(expr) => quote! { #expr },
            OptionCondition::Prefixed => quote! { #buffer_ident.read::<bool>()? }
        }
    }

    fn gen_write_condition(&self, field_ref: &TokenStream, buffer_ident: &Ident) -> Option<TokenStream> {
        match self {
            OptionCondition::Expr(_) => None,
            OptionCondition::Prefixed => Some(quote! { #buffer_ident.write(&#field_ref.is_some()); })
        }
    }
}

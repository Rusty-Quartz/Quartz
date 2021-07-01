pub mod packet;

use proc_macro2::{Span, TokenStream};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::{Error, GenericArgument, Ident, PathArguments, Result, Type};

pub fn the_crate() -> TokenStream {
    match crate_name("quartz") {
        Ok(FoundCrate::Itself) => quote! { crate },
        Ok(FoundCrate::Name(name)) => {
            let name = Ident::new(&name, Span::call_site());
            quote! { ::#name }
        }
        Err(e) => Error::new(Span::call_site(), format!("{}", e)).to_compile_error(),
    }
}

pub fn is_vec(ty: &Type) -> bool {
    match ty {
        Type::Path(path) =>
            path.qself.is_none()
                && path.path.leading_colon.is_none()
                && !path.path.segments.is_empty()
                && path.path.segments.last().unwrap().ident == "Vec",
        _ => false,
    }
}

pub fn is_option(ty: &Type) -> bool {
    match ty {
        Type::Path(path) =>
            path.qself.is_none()
                && path.path.leading_colon.is_none()
                && !path.path.segments.is_empty()
                && path.path.segments.last().unwrap().ident == "Option",
        _ => false,
    }
}

pub fn extract_type_from_container(ty: &Type) -> Result<Type> {
    match ty {
        Type::Path(path) => {
            let type_params = &path.path.segments.last().unwrap().arguments;

            let generic_arg = match type_params {
                PathArguments::AngleBracketed(params) => params.args.first().unwrap(),
                tokens @ _ => return Err(Error::new_spanned(tokens, "Expected type parameter")),
            };

            match generic_arg {
                GenericArgument::Type(ty) => Ok(ty.clone()),
                arg @ _ => Err(Error::new_spanned(arg, "Expected type parameter")),
            }
        }
        ty @ _ => Err(Error::new_spanned(ty, "Expected path type")),
    }
}

extern crate proc_macro;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Ident};

use proc_macro2::TokenStream;

#[proc_macro_derive(Listenable)]
pub fn derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let name = ast.ident;

    let enum_match = run_listeners_enum(&name, &ast.data);

    // Assembles and returns the method
    // TODO: add support for generics and where clauses possibly?
    proc_macro::TokenStream::from(quote! {
        use quartz_plugins::{PluginManager, Listeners};
        impl quartz_plugins::Listenable for #name {
            fn run_listeners(self, manager: &PluginManager) -> #name {
                match self {
                    #enum_match
                }
            }
        }
    })
}

fn to_snake_case(input: String) -> String {
    let mut output = String::new();
    for char in input.chars() {
        if char.is_uppercase() {
            output.push('_');
        }
        output.push(char.to_lowercase().next().unwrap())
    }
    output
}

fn run_listeners_enum(enum_name: &Ident, data: &Data) -> TokenStream {
    // TODO: Add support for more than just enums
    match *data {
        Data::Enum(ref data) => {
            let branch_names = data.variants.iter().map(|variant| {
                let variant_name = &variant.ident;
                quote! {
                    #enum_name::#variant_name{..}
                }
            });

            let branches = data.variants.iter().map(|variant| {
                let name = &variant.ident;
                let snake_name = to_snake_case(name.to_string());

                quote!{
                    manager.run_listeners::<Self>(Listeners::#name, self, format!("on{}", #snake_name))
                }

            });

            // Loops over the branch names and the branches to construct the entire match body
            quote! {
                #(#branch_names => #branches),*
            }
        }
        _ => unimplemented!("Currently only enums can derive Listener"),
    }
}

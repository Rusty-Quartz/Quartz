mod listenable;
use listenable::*;

extern crate proc_macro;

#[proc_macro_derive(Listenable)]
pub fn derive_listenable(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive_listenable_internal(input)
}

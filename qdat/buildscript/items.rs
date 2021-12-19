use std::{env, path::Path};

use indexmap::IndexMap;

use proc_macro2::TokenStream;
use serde::Deserialize;

use quote::{format_ident, quote};

use crate::buildscript::item_info::ItemInfo;

pub fn gen_items() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("items_output.rs");

    let data = serde_json::from_str::<IndexMap<String, RawItemData>>(include_str!(
        "../../assets/items.json"
    ))
    .expect("Error parsing items.json");

    let item_defs = gen_const_item_structs(&data);
    let usize_fn = gen_item_from_usize(&data);
    let uln_fn = gen_item_from_id(&data);

    std::fs::write(
        &dest_path,
        quote! {
            use phf::phf_map;
            #item_defs

            #usize_fn
            #uln_fn
        }
        .to_string(),
    )
    .unwrap();
    super::format_in_place(dest_path.as_os_str());

    println!("cargo:rerun-if-changed=../assets/items.json");
    println!("cargo:rerun-if-changed=buildscript/items.rs");
    println!("cargo:rerun-if-changed=buildscript/item_info.rs");
}

/// Generates a const variable for each vanilla item
fn gen_const_item_structs(data: &IndexMap<String, RawItemData>) -> TokenStream {
    let mut streams = Vec::new();

    for (i, (name, item)) in data.iter().enumerate() {
        let ident = format_ident!("{}_ITEM", name.to_uppercase());
        let stack_size = item.stack_size;
        let rarity = item.rarity;
        let num_id = i as u16;

        streams.push(if let Some(info) = &item.info {
            quote! {
                const #ident: Item = Item {
                    id: #name,
                    num_id: #num_id,
                    stack_size: #stack_size,
                    rarity: #rarity,
                    item_info: Some(#info)
                };
            }
        } else {
            quote! {
                const #ident: Item = Item {
                    id: #name,
                    num_id: #num_id,
                    stack_size: #stack_size,
                    rarity: #rarity,
                    item_info: None
                };
            }
        });
    }

    streams
        .into_iter()
        .reduce(|mut out, stream| {
            out.extend(stream);
            out
        })
        .unwrap()
}

/// Generates a phf map to lookup from the i32 used in the network protocol to an item instance
fn gen_item_from_usize(data: &IndexMap<String, RawItemData>) -> TokenStream {
    let mut branches = Vec::new();

    for (id, (name, _)) in data.iter().enumerate() {
        let name = format_ident!("{}_ITEM", name.to_uppercase());
        let id = id as i32;
        branches.push(quote! {
            #id => #name
        })
    }

    quote! {
        pub static ITEM_LOOKUP_BY_NUMERIC_ID: phf::Map<i32, Item> = phf_map!{
            #(#branches),*
        };
    }
}

/// Generates a phf map to lookup from an identifier to an Item instance
///
/// # Note
/// Is explicitly not a ULN, it is just the identifier part
fn gen_item_from_id(data: &IndexMap<String, RawItemData>) -> TokenStream {
    let mut branches = Vec::new();

    for (name, _) in data.iter() {
        let const_name = format_ident!("{}_ITEM", name.to_uppercase());
        branches.push(quote! {
            #name => #const_name
        })
    }

    quote! {
        pub static ITEM_LOOKUP_BY_NAME: phf::Map<&'static str, Item> = phf_map!{
            #(#branches),*
        };
    }
}

#[derive(Deserialize)]
struct RawItemData {
    pub stack_size: u8,
    pub rarity: u8,
    pub info: Option<ItemInfo>,
}

mod blockstate;
pub use blockstate::gen_blockstates;
mod item_info;
mod items;
pub use items::gen_items;

use std::{ffi::OsStr, process::Command};

pub(crate) fn format_in_place(file: &OsStr) {
    Command::new("rustfmt")
        .arg(file)
        .output()
        .unwrap_or_else(|_| panic!("Failed to format file: {file:?}"));
}

pub(crate) fn format_ast(code: String) -> syn::Result<String> {
    let file = syn::parse_file(&code)?;
    Ok(prettyplease::unparse(&file))
}

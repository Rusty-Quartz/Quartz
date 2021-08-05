mod blockstate;
pub use blockstate::gen_blockstates;

use std::{ffi::OsStr, process::Command};

pub(crate) fn format_in_place(file: &OsStr) {
    Command::new("rustfmt")
        .arg(file)
        .output()
        .expect(&format!("Failed to format file: {:?}", file));
}

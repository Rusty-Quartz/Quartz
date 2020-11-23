mod packets;
pub use packets::gen_packet_handlers;
mod blockstate;
pub use blockstate::gen_blockstates;
mod packet_types;
pub use packet_types::gen_packet_types;

use std::{ffi::OsStr, process::Command};

pub(crate) fn format_in_place(file: &OsStr) {
    Command::new("rustfmt")
        .arg(file)
        .output()
        .expect(&format!("Failed to format file: {:?}", file));
}

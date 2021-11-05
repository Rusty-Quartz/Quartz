#![allow(clippy::ptr_arg)]
mod buildscript;

fn main() {
    buildscript::gen_blockstates();
    buildscript::gen_items();
    println!("cargo:rerun-if-changed=build.rs");
}

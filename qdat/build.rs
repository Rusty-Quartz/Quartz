#![allow(clippy::ptr_arg)]
mod buildscript;

fn main() {
    buildscript::gen_blockstates();
    println!("cargo:rerun-if-changed=build.rs");
}

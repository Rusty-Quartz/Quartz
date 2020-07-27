mod buildscript;

fn main() {  
    buildscript::gen_packet_handlers();
    buildscript::gen_blockstates();
    println!("cargo:rerun-if-changed=build.rs");
}
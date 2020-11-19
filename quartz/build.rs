mod buildscript;

fn main() {  
    buildscript::gen_packet_handlers();
    buildscript::gen_blockstates();
    buildscript::gen_packet_types();
    println!("cargo:rerun-if-changed=build.rs");
}
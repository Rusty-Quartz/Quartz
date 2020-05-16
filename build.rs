use std::process::{ Command, Stdio };
use std::path::Path;

fn main() {
    Command::new("npm").args(&["run", "build"]).current_dir(&Path::new("./Pickaxe/")).status().unwrap();
    Command::new("node").stdout(Stdio::inherit()).arg("./Pickaxe/dist/Pickaxe.js").status().unwrap();
    println!("cargo:rerun-if-changed=Pickaxe/data/protocol.json");
    println!("cargo:rerun-if-changed=Pickaxe/data/mappings.json");

    println!("cargo:rerun-if-changed=Pickaxe/dist/Packet.js");
    println!("cargo:rerun-if-changed=Pickaxe/data/Pickaxe.js");

    println!("cargo:rerun-if-changed=build.rs");
    panic!("test")
}
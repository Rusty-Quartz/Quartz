use serde::{Deserialize};
use std::fs::File;
use std::io::Read;

#[derive(Deserialize)]
pub struct Config {
	pub max_players: u16,
	pub port: u16,
	pub motd: String
}

pub fn load_config(path: String) -> Config {
	let mut f = File::open(path).unwrap();
	let mut json = String::new();
	f.read_to_string(&mut json).unwrap();
	serde_json::from_str(&json).unwrap()
}
use serde::{Deserialize};
use std::fs::File;
use std::io::Read;

use crate::chat::component::Component;
use crate::component;

#[derive(Deserialize)]
pub struct Config {
    pub max_players: u16,
    pub port: u16,
    pub motd: String,
    #[serde(skip_deserializing)]
    pub motd_component: Component
}

pub fn load_config(path: String) -> Result<Config, String> {
    let mut file;
    match File::open(path) {
        Ok(f) => file = f,
        Err(e) => return Err(format!("Failed to open config file: {}", e))
    }

    let mut json = String::new();
    if let Err(e) = file.read_to_string(&mut json) {
        return Err(format!("Failed to read config file: {}", e));
    }
    
    let mut config: Config;
    match serde_json::from_str(&json) {
        Ok(cfg) => config = cfg,
        Err(e) => return Err(format!("Invalid config JSON: {}", e))
    }

    config.motd_component = component!(&config.motd)?;

    Ok(config)
}
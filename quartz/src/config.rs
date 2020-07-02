use std::fs::{File, OpenOptions};
use std::io::{prelude::*, Read, Write, SeekFrom};
use std::path::Path;

use serde::{Serialize, Deserialize, Serializer, Deserializer};

use log::*;

use chat::Component;
use chat::cfmt::parse_cfmt;

// The server config
#[derive(Serialize, Deserialize)]
pub struct Config {
    pub max_players: u16,
    pub server_ip: String,
    pub port: u16,
    #[serde(serialize_with = "Config::serialize_motd", deserialize_with = "Config::deserialize_motd")]
    pub motd: Component
}

// Custom ser/de functions
impl Config {
    fn serialize_motd<S>(_component: &Component, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer
    {
        serializer.serialize_str("A Minecraft Server")
    }

    fn deserialize_motd<'de, D>(deserializer: D) -> Result<Component, D::Error>
    where
        D: Deserializer<'de>
    {
        let cfmt: &'de str = Deserialize::deserialize(deserializer)?;

        match parse_cfmt(cfmt) {
            Ok(component) => Ok(component),
            Err(e) => {
                error!("Invalid MOTD format: {}", e);
                info!("Using default MOTD");
                Ok(Component::text("A Minecraft Server".to_owned()))
            }
        }
    }
}

// Instantiate a config with default values
impl Default for Config {
    fn default() -> Self {
        Config {
            max_players: 50,
            server_ip: "127.0.0.1".to_owned(),
            port: 25565,
            motd: Component::text("A Minecraft Server".to_owned())
        }
    }
}

pub fn load_config(path: String) -> Result<Config, String> {
    let std_path = Path::new(&path);

    if std_path.exists() {
        // Try to open the file
        let mut file;
        match OpenOptions::new().read(true).write(true).open(std_path) {
            Ok(f) => file = f,
            Err(e) => return Err(format!("Failed to open config file: {}", e))
        }
    
        // Read the file to a string
        let mut json = String::new();
        if let Err(e) = file.read_to_string(&mut json) {
            return Err(format!("Failed to read config file: {}", e));
        }
        
        // Parse the json
        let config: Config;
        match serde_json::from_str(&json) {
            Ok(cfg) => config = cfg,
            Err(e) => {
                error!("Invalid config JSON: {}", e);
                return use_default(&mut file);
            }
        }
    
        Ok(config)
    } else {
        info!("Config file not found, creating file");

        match File::create(std_path) {
            Ok(mut file) => use_default(&mut file),
            Err(e) => Err(format!("Failed to create config file: {}", e))
        }
    }
    
}

fn use_default(file: &mut File) -> Result<Config, String> {
    info!("Using default configurations");

    let default = Config::default();

    // Go to the beginning of the file
    if let Err(e) = file.seek(SeekFrom::Start(0)) {
        return Err(format!("Failed to write default config: {}", e));
    }

    // Write the default JSON
    let json = serde_json::to_string_pretty(&default).unwrap();
    let bytes = json.as_bytes();
    if let Err(e) = file.write_all(bytes) {
        return Err(format!("Failed to write default config: {}", e));
    }

    // Reset the file length
    if let Err(e) = file.set_len(bytes.len() as u64) {
        return Err(format!("Failed to write default config: {}", e));
    }

    Ok(default)
}
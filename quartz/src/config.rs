use std::fs::{File, OpenOptions};
use std::io::{self, prelude::*, Read, Write, SeekFrom};
use std::path::Path;
use chat::Component;
use chat::cfmt::parse_cfmt;
use log::*;
use serde::{Serialize, Deserialize, Serializer, Deserializer};

/// The main server configuration.
#[derive(Serialize, Deserialize)]
pub struct Config {
    /// The maximum number of players the server will allow, defaults to 50.
    pub max_players: u16,
    /// The server IP, defaults to "127.0.0.1"
    pub server_ip: String,
    /// The server port, defaults to 25565.
    pub port: u16,
    /// The server's message of the day, written using CFMT format (see `chat::cfmt::parse_cfmt`).
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

/// Attempts to parse the server configuration at the given path. The config should be in JSON format.
pub fn load_config(path: &Path) -> io::Result<Config> {
    let std_path = Path::new(path);

    if std_path.exists() {
        // Try to open the file
        let mut file = OpenOptions::new().read(true).write(true).open(std_path)?;
    
        // Read the file to a string
        let mut json = String::new();
        file.read_to_string(&mut json)?;
        
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
        use_default(&mut File::create(std_path)?)
    }
}

fn use_default(file: &mut File) -> io::Result<Config> {
    info!("Using default configurations");

    let default = Config::default();

    // Go to the beginning of the file
    file.seek(SeekFrom::Start(0))?;

    // Write the default JSON
    let json = serde_json::to_string_pretty(&default).unwrap();
    let bytes = json.as_bytes();
    file.write_all(bytes)?;

    // Reset the file length
    file.set_len(bytes.len() as u64)?;

    Ok(default)
}
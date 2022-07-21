use log::*;
use qdat::Gamemode;
use quartz_chat::Component;
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{self, prelude::*, Read, SeekFrom, Write},
    path::Path,
};

/// The main server configuration.
#[derive(Serialize, Deserialize)]
pub struct Config {
    /// The maximum number of players the server will allow, defaults to 50.
    pub max_players: u16,
    /// The server IP, defaults to "0.0.0.0"
    pub server_ip: String,
    /// The server port, defaults to 25565.
    pub port: u16,
    /// The server's message of the day, written using CFMT format (see `chat::cfmt::parse_cfmt`).
    pub motd: Component,
    /// Whether to run the server in online or offline mode
    /// Offline mode skips the login state of the connection flow
    pub online_mode: bool,
    /// The default gamemode for a player who joins the server
    pub default_gamemode: Gamemode,
}

// Instantiate a config with default values
impl Default for Config {
    fn default() -> Self {
        Config {
            max_players: 50,
            server_ip: "0.0.0.0".to_owned(),
            port: 25565,
            motd: Component::text("A Minecraft Server".to_owned()),
            online_mode: true,
            default_gamemode: Gamemode::Survival,
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
        let config: Config = match serde_json::from_str(&json) {
            Ok(cfg) => cfg,
            Err(e) => {
                error!("Invalid config JSON: {}", e);
                return use_default(&mut file);
            }
        };

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

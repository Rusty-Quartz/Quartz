use std::error::Error;
use std::path::Path;
use std::sync::Arc;
use linefeed::Interface;
use log::error;
use mcutil::logging;
use quartz::config::*;
use quartz::server::QuartzServer;

fn main() -> Result<(), Box<dyn Error>> {
    let console_interface = Arc::new(Interface::new("quartz-server")?);
    console_interface.set_prompt("> ")?;

    logging::init_logger("quartz", console_interface.clone())?;

    let config: Config;
    match load_config(Path::new("./config.json")) {
        Ok(cfg) => config = cfg,
        Err(e) => {
            error!("Failed to load config: {}", e);
            return Ok(())
        }
    }

    let mut server = QuartzServer::new(config, console_interface);
    
    server.init();
    server.run();
    drop(server);
    logging::cleanup();

    // Move off of the command prompt
    println!();

    Ok(())
}
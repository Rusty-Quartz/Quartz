use linefeed::Interface;
use log::error;
use quartz::{config::load_config, run, util::logging, Config};
use std::{error::Error, path::Path, sync::Arc};

fn main() -> Result<(), Box<dyn Error>> {
    let console_interface = Arc::new(Interface::new("quartz-server")?);
    console_interface.set_prompt("> ")?;

    logging::init_logger("quartz", console_interface.clone())?;

    let config: Config;
    match load_config(Path::new("./config.json")) {
        Ok(cfg) => config = cfg,
        Err(e) => {
            error!("Failed to load config: {}", e);
            return Ok(());
        }
    }

    run(config, console_interface);
    logging::cleanup();

    // Move off of the command prompt
    println!();

    Ok(())
}

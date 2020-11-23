use linefeed::Interface;
use log::error;
use quartz::{config::load_config, registry::StaticRegistry, util::logging, Config, QuartzServer};
use std::error::Error;
use std::path::Path;
use std::sync::Arc;

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

    let mut server: QuartzServer<StaticRegistry> = QuartzServer::new(config, console_interface);

    server.init();

    use quartz::world::chunk::ChunkProvider;
    use quartz::world::location::CoordinatePair;
    let mut provider: ChunkProvider<StaticRegistry> = ChunkProvider::new(
        "world",
        Path::new("/hd-pa/projects/FarLands2/run/world/region"),
        1,
    )?;
    provider.request_load_full(CoordinatePair::new(0, 0));

    server.run();
    drop(server);
    logging::cleanup();

    // Move off of the command prompt
    println!();

    Ok(())
}

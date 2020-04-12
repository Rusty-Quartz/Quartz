use crate::config::Config;
use serde::{Serialize};

pub struct QuartzServer {
    pub players: Vec<Player>,
    pub config: Config,
    pub debug: bool
}

#[derive(Serialize)]
pub struct Player {

}
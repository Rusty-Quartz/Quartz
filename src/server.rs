use crate::config::Config;
use serde::{Serialize};

pub struct RedstoneServer {
    pub players: Vec<Player>,
    pub config: Config,
    pub debug: bool
}

#[derive(Serialize)]
pub struct Player {

}
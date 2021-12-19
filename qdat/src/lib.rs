#![feature(new_uninit)]

pub mod block;
pub mod item;
pub mod world;

pub mod uln;

pub use uln::*;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename = "snake_case")]
pub enum Gamemode {
    /// None is only valid when sending the JoinGame packet previous gamemode packet
    None,
    Survival,
    Creative,
    Adventure,
    Specator,
}

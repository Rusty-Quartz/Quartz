use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct DimensionType {
    pub ultrawarm: bool,
    pub natural: bool,
    pub coordinate_scale: f32,
    pub has_skylight: bool,
    pub has_ceiling: bool,
    pub ambient_light: f32,
    pub fixed_time: Option<i32>,
    pub piglin_safe: bool,
    pub bed_works: bool,
    pub respawn_anchor_works: bool,
    pub has_raids: bool,
    pub logical_height: i32,
    pub min_y: i32,
    pub height: i32,
    pub infiniburn: String,
    #[serde(default = "Default::default")]
    pub effects: DimensionEffects,
}

#[derive(Serialize, Deserialize)]
pub enum DimensionEffects {
    #[serde(rename = "minecraft:overworld")]
    Overworld,
    #[serde(rename = "minecraft:the_nether")]
    TheNether,
    #[serde(rename = "minecraft:the_end")]
    TheEnd,
}

impl Default for DimensionEffects {
    fn default() -> Self {
        Self::Overworld
    }
}

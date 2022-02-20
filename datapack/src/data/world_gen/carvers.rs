use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize)]
pub struct CarverSettings {
    pub probability: f32,
    pub y: HeightProvider,
    #[serde(rename = "yScale")]
    pub y_scale: FloatProvider,
    pub default_settings: Option<HashMap<String, Value>>,
    pub lava_level: VerticalAnchor,
    #[serde(default = "Default::default")]
    pub aquifers_enabled: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "config")]
pub enum Carver {
    #[serde(rename = "minecraft:cave")]
    Cave {
        #[serde(flatten)]
        settings: CarverSettings,
        horizontal_radius_multiplier: FloatProvider,
        vertical_radius_multiplier: FloatProvider,
        floor_level: FloatProvider,
    },
    #[serde(rename = "minecraft:nether_cave")]
    NetherCave {
        #[serde(flatten)]
        settings: CarverSettings,
        horizontal_radius_multiplier: FloatProvider,
        vertical_radius_multiplier: FloatProvider,
        floor_level: FloatProvider,
    },
    #[serde(rename = "minecraft:underwater_cave")]
    UnderwaterCave {
        #[serde(flatten)]
        settings: CarverSettings,
        horizontal_radius_multiplier: FloatProvider,
        vertical_radius_multiplier: FloatProvider,
        floor_level: FloatProvider,
    },
    #[serde(rename = "minecraft:canyon")]
    Canyon {
        #[serde(flatten)]
        settings: CarverSettings,
        vertical_rotation: FloatProvider,
        shape: CanyonShape,
    },
    #[serde(rename = "minecraft:underwater_canyon")]
    UnderwaterCanyon {
        #[serde(flatten)]
        settings: CarverSettings,
        vertical_rotation: FloatProvider,
        shape: CanyonShape,
    },
}

#[derive(Serialize, Deserialize)]
pub struct CanyonShape {
    pub thickness: FloatProvider,
    pub width_smoothness: i32,
    pub horizontal_radius_factor: FloatProvider,
    pub vertical_radius_default_factor: FloatProvider,
    pub vertical_radius_center_factor: FloatProvider,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum FloatProvider {
    Constant(f32),
    Provider(FloatProviderInternal),
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum FloatProviderInternal {
    #[serde(rename = "minecraft:constant")]
    Constant(f32),
    #[serde(rename = "minecraft:uniform")]
    Uniform {
        min_inclusive: f32,
        max_exclusive: f32,
    },
    #[serde(rename = "minecraft:clamped_normal")]
    ClampedNormal {
        mean: f32,
        deviation: f32,
        min: f32,
        max: f32,
    },
    #[serde(rename = "minecraft:trapezoid")]
    Trapezoid { min: f32, max: f32, plateau: f32 },
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum HeightProvider {
    Constant {
        value: VerticalAnchor,
    },
    Uniform {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
    },
    BiasedToBottom {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
        /// # Note
        /// If present, has to be at least 1
        inner: Option<i32>,
    },
    VeryBiasedToBottom {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
        /// # Note
        /// If present, has to be at least 1
        inner: Option<i32>,
    },
    Trapezoid {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
        plateau: Option<i32>,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum VerticalAnchor {
    Absolute { absolute: i32 },
    AboveBottom { above_bottom: i32 },
    BelowTop { below_top: i32 },
}

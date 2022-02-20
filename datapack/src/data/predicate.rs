use std::collections::HashMap;

use crate::data::datatypes::*;
use qdat::UnlocalizedName;
use serde::{Deserialize, Serialize};

use super::loot_tables::NumberProvider;

#[derive(Serialize, Deserialize)]
#[serde(tag = "condition")]
pub enum Predicate {
    #[serde(rename = "minecraft:alternative")]
    Alternative { terms: Vec<Predicate> },
    #[serde(rename = "minecraft:block_state_property")]
    BlockStateProperty {
        block: UnlocalizedName,
        properties: Option<HashMap<String, String>>,
    },
    #[serde(rename = "minecraft:damage_source_properties")]
    DamageSourceProperties { predicate: Box<DamageType> },
    #[serde(rename = "minecraft:entity_properties")]
    EntityPropreties {
        entity: EntityType,
        predicate: Box<Entity>,
    },
    #[serde(rename = "minecraft:entity_scores")]
    EntityScores {
        entity: EntityType,
        scores: HashMap<String, AmountOrRange<i32>>,
    },
    #[serde(rename = "minecraft:inverted")]
    Inverted { term: Box<Predicate> },
    #[serde(rename = "minecraft:killed_by_player")]
    KilledByPlayer {
        #[serde(default)]
        inverse: bool,
    },
    #[serde(rename = "minecraft:location_check")]
    LocationCheck {
        #[serde(rename = "offsetX")]
        offset_x: Option<i32>,
        #[serde(rename = "offsetY")]
        offset_y: Option<i32>,
        #[serde(rename = "offsetZ")]
        offset_z: Option<i32>,
        predicate: PredicateLocation,
    },
    #[serde(rename = "minecraft:match_tool")]
    MatchTool { predicate: Item },
    #[serde(rename = "minecraft:random_chance")]
    RandomChance { chance: f32 },
    #[serde(rename = "minecraft:random_chance_with_looting")]
    RandomChanceWithLooting {
        chance: f32,
        looting_multiplier: f32,
    },
    #[serde(rename = "minecraft:reference")]
    Reference { name: UnlocalizedName },
    #[serde(rename = "minecraft:survives_explosion")]
    SurvivesExplosion,
    #[serde(rename = "minecraft:table_bonus")]
    TableBonus {
        enchantment: UnlocalizedName,
        chances: Vec<f32>,
    },
    #[serde(rename = "minecraft:time_check")]
    TimeCheck {
        value: NumberProvider<i32>,
        period: i32,
    },
    #[serde(rename = "minecraft:weather_check")]
    WeatherCheck { raining: bool, thundering: bool },
    #[serde(rename = "minecraft:value_check")]
    ValueCheck {
        value: NumberProvider<i32>,
        range: AmountOrRange<NumberProvider<i32>>,
    },
}

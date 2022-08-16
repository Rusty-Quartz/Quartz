use std::collections::HashMap;

use qdat::UnlocalizedName;
use serde::{Deserialize, Serialize};

use crate::data::tags::IdsOrTag;

use super::{features::Direction, loot_tables::NumberProvider, noise_settings::BlockState};

/// A condition that matches on an amount of slots
#[derive(Serialize, Deserialize)]
pub struct Slots {
    pub empty: Option<AmountOrRange<i32>>,
    pub full: Option<AmountOrRange<i32>>,
    pub occupied: Option<AmountOrRange<i32>>,
}

/// A condition that matches on a instance of damage
#[derive(Serialize, Deserialize)]
pub struct Damage {
    pub blocked: Option<bool>,
    pub dealt: Option<AmountOrRange<f64>>,
    pub source_entity: Option<Entity>,
    pub taken: Option<Range<f64>>,
}

/// A condition that matches on a certain damage type
#[derive(Serialize, Deserialize)]
pub struct DamageType {
    pub bypasses_armor: Option<bool>,
    pub bypasses_invulerability: Option<bool>,
    pub bypasses_magic: Option<bool>,
    pub direct_entity: Option<Entity>,
    pub is_explosion: Option<bool>,
    pub is_fire: Option<bool>,
    pub is_magic: Option<bool>,
    pub is_projectile: Option<bool>,
    pub is_lightning: Option<bool>,
    pub source_entity: Option<Entity>,
}

/// A condition that matches a on a certain location
#[derive(Serialize, Deserialize)]
pub struct Location {
    pub biome: Option<String>,
    pub block: Option<LocationBlock>,
}

/// Part of a condition that matches on a block
#[derive(Serialize, Deserialize)]
pub struct LocationBlock {
    pub blocks: Option<Vec<UnlocalizedName>>,
    pub tag: Option<String>,
    pub nbt: Option<String>,
    pub state: Option<HashMap<String, String>>,
}

/// Part of a condition that matches on an entity
#[derive(Serialize, Deserialize)]
pub struct Entity {
    pub distance: Option<Distance<f32>>,
    pub effects: Option<HashMap<String, StatusEffect>>,
    pub equipment: Option<Equipment>,
    pub flags: Option<EntityFlags>,
    pub lightning_bolt: Option<LightningBolt>,
    pub nbt: Option<String>,
    pub pasenger: Option<Box<Entity>>,
    pub player: Option<Player>,
    pub stepping_on: Option<Location>,
    pub team: Option<String>,
    pub r#type: Option<String>,
    pub targeted_entity: Option<Box<Entity>>,
    pub vehicle: Option<Box<Entity>>,
    pub location: Option<PredicateLocation>,
}

#[derive(Serialize, Deserialize)]
pub struct Player {
    pub looking_at: Option<Box<Entity>>,
    pub advancements: Option<HashMap<String, HashMap<String, bool>>>,
    pub gamemode: Option<String>,
    pub level: Option<AmountOrRange<i32>>,
    pub recipes: Option<HashMap<String, bool>>,
    pub stats: Option<Statistic>,
}

#[derive(Serialize, Deserialize)]
pub struct Statistic {
    pub r#type: Option<String>,
    pub stat: Option<String>,
    pub value: Option<AmountOrRange<i32>>,
}

#[derive(Serialize, Deserialize)]
pub struct LightningBolt {
    pub blocks_set_on_fire: Option<i32>,
    pub entity_struct: Option<Box<Entity>>,
}

#[derive(Serialize, Deserialize)]
pub struct EntityFlags {
    pub is_on_fire: Option<bool>,
    pub is_sneaking: Option<bool>,
    pub is_sprinting: Option<bool>,
    pub is_baby: Option<bool>,
}

#[derive(Serialize, Deserialize)]
pub struct Equipment {
    pub mainhand: Option<Item>,
    pub offhand: Option<Item>,
    pub head: Option<Item>,
    pub chest: Option<Item>,
    pub legs: Option<Item>,
    pub feet: Option<Item>,
}

#[derive(Serialize, Deserialize)]
pub struct StatusEffect {
    pub ambient: Option<bool>,
    pub amplifier: Option<AmountOrRange<i32>>,
    pub duration: Option<AmountOrRange<i32>>,
    pub visible: Option<bool>,
}

#[derive(Serialize, Deserialize)]
pub struct Distance<T> {
    pub absolute: Option<Range<T>>,
    pub horizontal: Option<Range<T>>,
    pub x: Option<Range<T>>,
    pub y: Option<Range<T>>,
    pub z: Option<Range<T>>,
}

#[derive(Serialize, Deserialize)]
pub struct Range<T> {
    pub max: Option<T>,
    pub min: Option<T>,
}


#[derive(Serialize, Deserialize)]
pub struct Item {
    pub count: Option<AmountOrRange<i32>>,
    pub durability: Option<AmountOrRange<i32>>,
    pub enchantments: Option<Vec<Enchantment>>,
    pub stored_enchantments: Option<Vec<Enchantment>>,
    pub items: Option<Vec<UnlocalizedName>>,
    // TODO: have Cassy impl a way to have serde parse snbt
    pub nbt: Option<String>,
    pub potion: Option<UnlocalizedName>,
    pub tag: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum AmountOrRange<T> {
    Amount(T),
    Range(Range<T>),
}

#[derive(Serialize, Deserialize)]
pub struct Enchantment {
    pub enchantment: Option<UnlocalizedName>,
    pub levels: Option<AmountOrRange<i32>>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    This,
    Killer,
    DirectKiller,
    KillerPlayer,
}

#[derive(Serialize, Deserialize)]
pub struct Position {
    pub x: Option<AmountOrRange<f64>>,
    pub y: Option<AmountOrRange<f64>>,
    pub z: Option<AmountOrRange<f64>>,
}

#[derive(Serialize, Deserialize)]
pub struct PredicateLocation {
    pub position: Option<Position>,
    pub biome: Option<UnlocalizedName>,
    pub feature: Option<LocationFeature>,
    pub dimension: Option<UnlocalizedName>,
    pub light: Option<LocationLight>,
    pub smokey: Option<bool>,
    pub block: Option<LocationBlock>,
    pub fluid: Option<LocationFluid>,
}

#[derive(Serialize, Deserialize)]
pub struct LocationLight {
    pub light: NumberProvider<f32>,
}

#[derive(Serialize, Deserialize)]
pub struct LocationFluid {
    pub fluid: Option<UnlocalizedName>,
    pub tag: Option<UnlocalizedName>,
    // Should be also number, object, boolean
    pub state: Option<HashMap<String, String>>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocationFeature {
    Unset,
    BastionRemnant,
    BuriedTreasure,
    DesertPyramid,
    EndCity,
    Fortress,
    Igloo,
    JunglePyramid,
    Mansion,
    Mineshaft,
    Monument,
    NetherFossil,
    OceanRuin,
    PillagerOutpost,
    RuinedPortal,
    Shipwreck,
    Stronghold,
    SwampHut,
    Village,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BlockPredicate {
    #[serde(rename = "minecraft:all_of")]
    AllOf { predicates: Vec<BlockPredicate> },
    #[serde(rename = "minecraft:any_of")]
    AnyOf { predicates: Vec<BlockPredicate> },
    #[serde(rename = "minecraft:has_sturdy_face")]
    HasSturdyFace {
        offset: Option<[i8; 3]>,
        direction: Direction,
    },
    #[serde(rename = "minecraft:inside_world_bounds")]
    InsideWorldBounds { offset: Option<[i8; 3]> },
    #[serde(rename = "minecraft:matching_block_tag")]
    MatchingBlockTag {
        offset: Option<[i8; 3]>,
        tag: UnlocalizedName,
    },
    #[serde(rename = "minecraft:matching_blocks")]
    MatchingBlocks {
        offset: Option<[i8; 3]>,
        blocks: IdsOrTag,
    },
    #[serde(rename = "minecraft:matching_fluids")]
    MatchingFluids {
        offset: Option<[i8; 3]>,
        fluids: IdsOrTag,
    },
    #[serde(rename = "minecraft:not")]
    Not { predicate: Box<BlockPredicate> },
    #[serde(rename = "minecraft:replaceable")]
    Replaceable,
    #[serde(rename = "minecraft:solid")]
    Solid,
    #[serde(rename = "minecraft:true")]
    True,
    #[serde(rename = "minecraft:would_survive")]
    WouldSurvive {
        offset: Option<[i8; 3]>,
        state: BlockState,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum BlockList {
    Singleton(UnlocalizedName),
    List(Vec<UnlocalizedName>),
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum ValueOrList<T> {
    Value(T),
    List(Vec<T>),
}

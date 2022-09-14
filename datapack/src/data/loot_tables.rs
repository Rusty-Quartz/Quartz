use std::collections::HashMap;

use qdat::UnlocalizedName;
use serde::{Deserialize, Serialize};

use crate::data::{
    datatypes::{AmountOrRange, Enchantment, Entity, EntityType, PredicateLocation},
    predicate::Predicate,
};

#[derive(Serialize, Deserialize)]
pub struct LootTable {
    pub r#type: LootTableType,
    #[serde(default)]
    pub functions: Vec<LootTableFunction>,
    #[serde(default)]
    pub pools: Vec<LootTablePool>,
}

#[derive(Serialize, Deserialize)]
pub enum LootTableType {
    #[serde(rename = "minecraft:empty")]
    Empty,
    #[serde(rename = "minecraft:entity")]
    Entity,
    #[serde(rename = "minecraft:block")]
    Block,
    #[serde(rename = "minecraft:chest")]
    Chest,
    #[serde(rename = "minecraft:fishing")]
    Fishing,
    #[serde(rename = "minecraft:gift")]
    Gift,
    #[serde(rename = "minecraft:advancement_reward")]
    AdvancementReward,
    #[serde(rename = "minecraft:barter")]
    Barter,
    #[serde(rename = "minecraft:command")]
    Command,
    #[serde(rename = "minecraft:selector")]
    Selector,
    #[serde(rename = "minecraft:advancement_entity")]
    AdvancementEntity,
    #[serde(rename = "minecraft:generic")]
    Generic,
}

#[derive(Serialize, Deserialize)]
pub struct LootTableFunction {
    pub function: UnlocalizedName,
    #[serde(default)]
    pub conditions: Vec<Predicate>,
}

#[derive(Serialize, Deserialize)]
pub struct LootTablePool {
    #[serde(default)]
    pub conditions: Vec<Predicate>,
    #[serde(default)]
    pub functions: Vec<LootTableFunction>,
    pub rolls: NumberProvider<f32>,
    pub bonus_rolls: NumberProvider<f32>,
    #[serde(default)]
    pub entries: Vec<LootTableEntry>,
}

#[derive(Serialize, Deserialize)]
pub struct LootTableEntry {
    #[serde(default)]
    pub conditions: Vec<Predicate>,
    #[serde(default)]
    pub functions: Vec<LootTableFunction>,
    #[serde(flatten)]
    pub r#type: LootTableEntryType,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "condition")]
pub enum LootTableCondition {
    #[serde(rename = "minecraft:alternative")]
    Alternative { terms: Vec<LootTableCondition> },
    #[serde(rename = "minecraft:block_state_property")]
    BlockStateProperty {
        block: UnlocalizedName,
        properties: HashMap<String, String>,
    },
    #[serde(rename = "minecraft:entity_properties")]
    EntityProperties {
        // This has to be "this"
        entity: String,
        predicate: Box<Entity>,
    },
    #[serde(rename = "minecraft:entity_scores")]
    EntityScores {
        // Has to be "this"
        entity: String,
        scores: HashMap<String, NumberProvider<f32>>,
    },
    #[serde(rename = "minecraft:inverted")]
    Inverted { term: Predicate },
    #[serde(rename = "minecraft:location_check")]
    LocationCheck {
        #[serde(rename = "offsetX")]
        x_offset: Option<f32>,
        #[serde(rename = "offsetY")]
        y_offset: Option<f32>,
        #[serde(rename = "offsetZ")]
        z_offset: Option<f32>,
        predicate: Option<Box<PredicateLocation>>,
    },
    #[serde(rename = "minecraft:match_tool")]
    MatchTool {
        #[serde(default)]
        items: Vec<UnlocalizedName>,
        tag: Option<UnlocalizedName>,
        count: Option<AmountOrRange<f32>>,
        durability: Option<AmountOrRange<f32>>,
        potion: Option<UnlocalizedName>,
        nbt: Option<String>,
        enchantments: Option<Enchantment>,
    },
    #[serde(rename = "minecraft:random_chance")]
    RandomChance { chance: f32 },
    #[serde(rename = "minecraft:reference")]
    Reference { name: UnlocalizedName },
    #[serde(rename = "minecraft:survives_explosion")]
    SurvivesExplosion,
    #[serde(rename = "minecraft:table_bonus")]
    TableBonus {
        enchantment: UnlocalizedName,
        #[serde(default)]
        chances: Vec<f32>,
    },
    #[serde(rename = "minecraft:time_check")]
    TimeCheck {
        value: Option<NumberProvider<f32>>,
        period: Option<i32>,
    },
    #[serde(rename = "minecraft:value_check")]
    ValueCheck {
        value: Option<NumberProvider<f32>>,
        range: Option<NumberProvider<f32>>,
    },
    #[serde(rename = "minecraft:weather_check")]
    WeatherCheck {
        raining: Option<bool>,
        thundering: Option<bool>,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LootTableEntryType {
    #[serde(rename = "minecraft:item")]
    Item { name: UnlocalizedName },
    #[serde(rename = "minecraft:tag")]
    Tag {
        name: UnlocalizedName,
        #[serde(default)]
        expand: bool,
    },
    #[serde(rename = "minecraft:loot_table")]
    LootTable { name: UnlocalizedName },
    #[serde(rename = "minecraft:group")]
    Group { childern: Vec<Box<LootTableEntry>> },
    #[serde(rename = "minecraft:alternatives")]
    Alternatives { children: Vec<Box<LootTableEntry>> },
    #[serde(rename = "minecraft:sequence")]
    Sequence { children: Vec<Box<LootTableEntry>> },
    #[serde(rename = "minecraft:dynamic")]
    Dynamic { name: UnlocalizedName },
    #[serde(rename = "minecraft:empty")]
    Empty,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum NumberProvider<T> {
    Singleton(T),
    Object(NumberProviderInternal<T>),
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NumberProviderInternal<T> {
    #[serde(rename = "minecraft:constant")]
    Constant { value: T },
    #[serde(rename = "minecraft:uniform")]
    Uniform {
        max: Box<NumberProvider<T>>,
        min: Box<NumberProvider<T>>,
    },
    #[serde(rename = "minecraft:binomial")]
    Binomial {
        n: Box<NumberProvider<i32>>,
        p: Box<NumberProvider<f32>>,
    },
    #[serde(rename = "minecraft:score")]
    Score {
        target: ScoreboardNumProviderTarget,
        score: String,
        scale: Option<f32>,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum ScoreboardNumProviderTarget {
    Constant(EntityType),
    Variable(ScoreboardNameProvider),
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ScoreboardNameProvider {
    Fixed { name: String },
    Context { target: EntityType },
}

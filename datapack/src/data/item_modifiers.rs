use std::collections::HashMap;

use qdat::UnlocalizedName;
use serde::{Deserialize, Serialize};

use super::{
    datatypes::AmountOrRange,
    loot_tables::NumberProvider,
    noise_settings::BlockState,
    predicate::Predicate,
};

#[derive(Serialize, Deserialize)]
pub struct ItemModifier {
    #[serde(flatten)]
    modifier: ItemModifierType,
    conditions: Vec<Predicate>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemModifierType {
    ApplyBonus {
        enchantment: UnlocalizedName,
        formula: String,
        parameters: ApplyBonusParameters,
    },
    CopyName {
        source: String,
    },
    CopyNbt {
        source: NbtSource,
        ops: Vec<NbtOp>,
    },
    CopyState(BlockState),
    EnchantRandomly {
        enchantments: Vec<UnlocalizedName>,
    },
    EnchantWithLevels {
        treasure: bool,
        levels: NumberProvider<i32>,
    },
    ExplorationMap {
        destination: String,
        decoration: String,
        zoom: Option<i32>,
        search_results: Option<i32>,
        skip_existing_chunks: Option<bool>,
    },
    ExplosionDelay,
    FurnaceSmelt,
    FillPlayerHead {
        entity: String,
    },
    LimitCount {
        limit: AmountOrRange<NumberProvider<i32>>,
    },
    LootingEnchant {
        count: NumberProvider<i32>,
        limit: i32,
    },
    SetAttributes {
        modifiers: Vec<AttributeModifier>,
    },
    SetBannerPatern {
        patterns: Vec<BannerPattern>,
        #[serde(default = "Default::default")]
        append: bool,
    },
    SetContents {
        // NOTE: no clue what datatype this is lol
        entries: Vec<String>,
    },
    SetCount {
        count: NumberProvider<i32>,
        #[serde(default = "Default::default")]
        add: bool,
    },
    SetDamage {
        damage: NumberProvider<f32>,
        #[serde(default = "Default::default")]
        add: bool,
    },
    SetEnchantments {
        enchantments: HashMap<UnlocalizedName, NumberProvider<i32>>,
        #[serde(default = "Default::default")]
        add: bool,
    },
    SetLootTable {
        name: UnlocalizedName,
        #[serde(default = "Default::default")]
        seed: i32,
    },
    SetLore {
        // TODO: this is a json text component
        lore: Vec<String>,
        entry: String,
        replace: bool,
    },
    SetName {
        // TODO: this is a json text component
        name: String,
        entity: String,
    },
    SetNbt {
        tag: String,
    },
    SetStewEffect {
        effects: Vec<StewEffect>,
    },
}

#[derive(Serialize, Deserialize)]
pub struct ApplyBonusParameters {
    extra: i32,
    probability: f32,
    #[serde(rename = "bonusMultiplier")]
    bonus_multiplier: f32,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "type")]
pub enum NbtSource {
    Context { target: String },
    Storage { source: UnlocalizedName },
}

#[derive(Serialize, Deserialize)]
pub struct NbtOpData {
    source: String,
    target: String,
    op: NbtOp,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NbtOp {
    Replace,
    Append,
    Merge,
}

#[derive(Serialize, Deserialize)]
pub struct AttributeModifier {
    name: String,
    atrribute: String,
    operation: AttributeModifierOperation,
    amount: NumberProvider<f32>,
    id: Option<String>,
    slot: AttributeModifierSlot,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributeModifierOperation {
    Addition,
    MultiplyBase,
    MultiplyTotal,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum AttributeModifierSlots {
    Singleton(AttributeModifierSlot),
    List(Vec<AttributeModifierSlot>),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttributeModifierSlot {
    MainHand,
    OffHand,
    Feet,
    Legs,
    Chest,
    Head,
}

#[derive(Serialize, Deserialize)]
pub struct BannerPattern {
    pattern: String,
    color: BannerColor,
}

#[derive(Serialize, Deserialize)]
pub enum BannerColor {
    White,
    Orange,
    Magenta,
    LightBlue,
    Yellow,
    Lime,
    Pink,
    Gray,
    LightGray,
    Cyan,
    Purple,
    Blue,
    Brown,
    Green,
    Red,
    Black,
}

#[derive(Serialize, Deserialize)]
pub struct StewEffect {
    r#type: UnlocalizedName,
    duration: NumberProvider<i32>,
}

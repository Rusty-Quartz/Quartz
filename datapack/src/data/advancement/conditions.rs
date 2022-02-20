use std::collections::HashMap;

use qdat::UnlocalizedName;
use serde::{Deserialize, Serialize};

use crate::data::{
    datatypes::{
        AmountOrRange,
        Damage,
        DamageType,
        Distance,
        Entity,
        Item,
        Location,
        PredicateLocation,
        Slots,
        StatusEffect,
    },
    predicate::Predicate,
};


// TODO: a lot of fields that are entities can also be predicates
// We need some kind of enum or a system for allowing the fields to be predicates too
// We just don't support predicates yet so its not worth putting effort into yet
/// The different types of triggers & conditions in an [Advancement](super::Advancement)
#[derive(Serialize, Deserialize)]
#[serde(tag = "trigger", content = "conditions")]
pub enum AdvancementConditions {
    #[serde(rename = "minecraft:bee_nest_destroyed")]
    BeeNestDestroyed {
        block: Option<UnlocalizedName>,
        item: Option<Item>,
        num_bees: Option<i32>,
        playre: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:bred_animals")]
    BredAnimals {
        child: Option<ConditionEntity>,
        parent: Option<ConditionEntity>,
        partner: Option<ConditionEntity>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:brewed_potion")]
    BrewedPotion {
        potion: Option<String>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:changed_dimension")]
    ChangedDimension {
        from: Option<String>,
        to: Option<String>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:channeled_lightning")]
    ChanneledLightning {
        victims: Option<Vec<ConditionEntity>>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:construct_beacon")]
    ConstructBeacon {
        level: Option<AmountOrRange<i32>>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:consume_item")]
    ConsumeItem {
        item: Option<Item>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:cured_zombie_villager")]
    CuredZombieVillager {
        villager: Option<ConditionEntity>,
        zombie: Option<ConditionEntity>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:effects_changed")]
    EffectsChanged {
        // NOTE: Technically this shouldn't allow ambient or visible
        // But I can't be bothered to make another struct
        effects: Option<HashMap<String, StatusEffect>>,
        source: Option<ConditionEntity>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:enchanted_item")]
    EnchantedItem {
        item: Option<Item>,
        levels: Option<AmountOrRange<i32>>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:enter_block")]
    EnterBlock {
        block: Option<String>,
        // TODO: the value of the map needs to be an enum over the possible values in a blockstate
        // This can be anything that can be held in a blockstate
        // Mojang... I hate you
        state: Option<HashMap<String, String>>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:entity_hurt_player")]
    EntityHurtPlayer {
        damage: Option<Damage>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:entity_killed_player")]
    EntityKilledPlayer {
        entity: Option<ConditionEntity>,
        killing_blow: Option<DamageType>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:fall_from_height")]
    FallFromHeight {
        player: Option<ConditionEntity>,
        start_position: PredicateLocation,
        distance: Distance<f32>,
    },
    #[serde(rename = "minecraft:filled_bucket")]
    FilledBucket {
        item: Option<Item>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:fishing_rod_hooked")]
    FishingRodHooked {
        entity: Option<ConditionEntity>,
        item: Option<Item>,
        rod: Option<Item>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:hero_of_the_village")]
    HeroOfTheVillage {
        location: Option<Location>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:impossible")]
    Impossible,
    #[serde(rename = "minecraft:inventory_changed")]
    InventoryChanged {
        items: Option<Vec<Item>>,
        slots: Option<Slots>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:item_durability_changed")]
    ItemDurabilityChanged {
        delta: Option<AmountOrRange<i32>>,
        durability: Option<AmountOrRange<i32>>,
        item: Option<Item>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:item_used_on_block")]
    ItemUsedOnBlock {
        location: Option<Location>,
        item: Option<Item>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:killed_by_crossbow")]
    KilledByCrossbow {
        unique_entity_types: Option<AmountOrRange<i32>>,
        victims: Option<Vec<ConditionEntity>>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:levitation")]
    Levitation {
        distance: Option<Distance<f32>>,
        duration: Option<AmountOrRange<i32>>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:lightning_strike")]
    LightningStrike {
        lightning: Option<ConditionEntity>,
        bystander: Option<ConditionEntity>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:location")]
    Location {
        location: Option<Location>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:nether_travel")]
    NetherTravel {
        entered: Option<Location>,
        exited: Option<Location>,
        distance: Option<Distance<f32>>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:placed_block")]
    PlacedBlock {
        block: Option<String>,
        item: Option<Item>,
        location: Option<Location>,
        // Same note as with EnterBlock
        state: Option<HashMap<String, String>>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:player_generates_container_loot")]
    PlayerGeneratesContainerLoot {
        loot_table: Option<String>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:player_hurt_entity")]
    PlayerHurtEntity {
        damage: Option<Damage>,
        entity: Option<ConditionEntity>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:player_interacted_with_entity")]
    PlayerInteractedWithEntity {
        item: Option<Item>,
        entity: Option<ConditionEntity>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:player_killed_entity")]
    PlayerKilledEntity {
        entity: Option<ConditionEntity>,
        killing_blow: Option<DamageType>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:recipe_unlocked")]
    RecipeUnlocked {
        recipe: Option<String>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:ride_entity_in_lava")]
    RideEntityInLava {
        player: Option<ConditionEntity>,
        distance: Option<Distance<f32>>,
    },
    #[serde(rename = "minecraft:shot_crossbow")]
    ShotCrossbow {
        item: Option<Item>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:slept_in_bed")]
    SleptInBed {
        location: Option<Location>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:slide_down_block")]
    SlideDownBlock {
        block: Option<String>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:started_riding")]
    StartedRiding { player: ConditionEntity },
    #[serde(rename = "minecraft:summoned_entity")]
    SummonedEntity {
        entity: Option<ConditionEntity>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:tame_animal")]
    TameAnimal {
        entity: Option<ConditionEntity>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:target_hit")]
    TargetHit {
        signal_strenght: Option<i32>,
        projectile: Option<ConditionEntity>,
        shooter: Option<ConditionEntity>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:thrown_item_picked_up_by_entity")]
    ThrownItemPickedUpByEntity {
        item: Option<Item>,
        entity: Option<ConditionEntity>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:tick")]
    Tick { player: ConditionEntity },
    #[serde(rename = "minecraft:used_ender_eye")]
    UsedEnderEye {
        distance: Option<AmountOrRange<f64>>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:used_totem")]
    UsedTotem {
        item: Option<Item>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:using_item")]
    UsingItem {
        item: Option<Item>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:villager_trade")]
    VillagerTrade {
        item: Option<Item>,
        villager: Option<ConditionEntity>,
        player: Option<ConditionEntity>,
    },
    #[serde(rename = "minecraft:voluntary_exile")]
    VoluntaryExile {
        location: Option<Location>,
        player: Option<ConditionEntity>,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConditionEntity {
    Legacy(Entity),
    Predicate(Vec<Predicate>),
}

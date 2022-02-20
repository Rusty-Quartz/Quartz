use qdat::UnlocalizedName;
use serde::{Deserialize, Serialize};

use crate::data::recipe::{
    cooking::*,
    shaped::ShapedCraftingRecipe,
    shapeless::ShapelessCraftingRecipe,
    smithing::SmithingRecipe,
    stonecutting::StonecuttingRecipe,
};


/// The output type of most recipe types
#[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
pub struct RecipeOutput {
    pub item: UnlocalizedName,
    #[serde(default = "default_count")]
    pub count: u8,
}

const fn default_count() -> u8 {
    1
}

/// The vanilla recipe types, returns Other if the type is unknown
/// # Note
/// Singleton variants are not defined in datapacks and are left to servers / clients to implement properly<br>
/// Recipes in datapacks with these types only either enable or disable that special recipe type
#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(tag = "type")]
pub enum VanillaRecipeType {
    #[serde(rename = "minecraft:crafting_shaped")]
    ShapedRecipe(ShapedCraftingRecipe),
    #[serde(rename = "minecraft:crafting_shapeless")]
    ShapelessRecipe(ShapelessCraftingRecipe),
    #[serde(rename = "minecraft:smelting")]
    SmeltingRecipe(CookingRecipe<SmeltingRecipe>),
    #[serde(rename = "minecraft:blasting")]
    BlastingRecipe(CookingRecipe<BlastingRecipe>),
    #[serde(rename = "minecraft:smoking")]
    SmokingRecipe(CookingRecipe<SmokingRecipe>),
    #[serde(rename = "minecraft:campfire_cooking")]
    CampfireRecipe(CookingRecipe<CampfireRecipe>),
    #[serde(rename = "minecraft:smithing")]
    SmithingRecipe(SmithingRecipe),
    #[serde(rename = "minecraft:stonecutting")]
    StonecuttingRecipe(StonecuttingRecipe),

    // Special recipe types
    #[serde(rename = "minecraft:armordye")]
    ArmorDye,
    #[serde(rename = "minecraft:bannerduplicate")]
    BannerDuplicate,
    #[serde(rename = "minecraft:bookcloning")]
    BookClone,
    #[serde(rename = "minecraft:firework_rocket")]
    FireworkRocket,
    #[serde(rename = "minecraft:firework_star")]
    FireworkStar,
    #[serde(rename = "minecraft:firework_star_fade")]
    FireworkStarFade,
    #[serde(rename = "minecraft:mapcloning")]
    MapClone,
    #[serde(rename = "minecraft:mapextending")]
    MapExtend,
    #[serde(rename = "minecraft:repairitem")]
    RepairItem,
    #[serde(rename = "minecraft:shielddecoration")]
    ShieldDecorate,
    #[serde(rename = "minecraft:shulkerboxcoloring")]
    ShulkerBoxColoring,
    #[serde(rename = "minecraft:tippedarrow")]
    TippedArrow,
    #[serde(rename = "minecraft:suspiciousstew")]
    SuspiciousStew,

    // If we get unknown types
    #[serde(other)]
    Unknown,
}

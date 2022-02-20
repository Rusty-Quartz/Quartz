use qdat::UnlocalizedName;
use serde::{Deserialize, Serialize};

use crate::data::recipe::ingredient::Ingredient;

/// A smithing recipe
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct SmithingRecipe {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// The base of the smithing recipe
    /// # Note
    /// This ingredient cannot be a list
    pub base: Ingredient,
    pub addition: Ingredient,
    /// In most cases, the nbt of enchantments will be carried over to the result
    pub result: SmithingOutput,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
// mojang I hate you please stop making things very close but not the same
pub struct SmithingOutput {
    pub item: UnlocalizedName,
}

#[test]
fn smithing_ser_test() {
    use crate::data::recipe::{ingredient::Ingredient, VanillaRecipeType};

    let recipe = VanillaRecipeType::SmithingRecipe(SmithingRecipe {
        group: None,
        base: Ingredient::Item(UnlocalizedName::minecraft("diamond_pickaxe")),
        addition: Ingredient::Item(UnlocalizedName::minecraft("netherite_ingot")),
        result: SmithingOutput {
            item: UnlocalizedName::minecraft("netherite_pickaxe"),
        },
    });

    let serialized = serde_json::to_string(&recipe).unwrap();

    assert_eq!(
        &serialized,
        r#"{"type":"minecraft:smithing","base":{"item":"minecraft:diamond_pickaxe"},"addition":{"item":"minecraft:netherite_ingot"},"result":{"item":"minecraft:netherite_pickaxe"}}"#
    )
}

#[test]
fn smithing_de_test() {
    use crate::data::recipe::{ingredient::Ingredient, VanillaRecipeType};

    let input = r#"{
        "type": "minecraft:smithing",
        "base": {
          "item": "minecraft:diamond_pickaxe"
        },
        "addition": {
          "item": "minecraft:netherite_ingot"
        },
        "result": {
          "item": "minecraft:netherite_pickaxe"
        }
      }"#;

    let recipe: VanillaRecipeType = serde_json::from_str(input).unwrap();

    assert_eq!(
        recipe,
        VanillaRecipeType::SmithingRecipe(SmithingRecipe {
            group: None,
            base: Ingredient::Item(UnlocalizedName::minecraft("diamond_pickaxe")),
            addition: Ingredient::Item(UnlocalizedName::minecraft("netherite_ingot")),
            result: SmithingOutput {
                item: UnlocalizedName::minecraft("netherite_pickaxe")
            }
        })
    )
}

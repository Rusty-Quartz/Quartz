use qdat::UnlocalizedName;
use serde::{de::Visitor, ser::SerializeMap, Deserialize, Serialize};

use crate::data::recipe::{ingredient::Ingredient, recipe::RecipeOutput};

/// A stone cutting recipe
#[derive(Debug, Eq, PartialEq)]
pub struct StonecuttingRecipe {
    pub group: Option<String>,
    pub input: Ingredient,
    pub result: RecipeOutput,
}

impl Serialize for StonecuttingRecipe {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        let mut map = serializer.serialize_map(Some(4))?;

        if self.group.is_some() {
            map.serialize_entry("group", &self.group)?;
        }

        map.serialize_entry("ingredient", &self.input)?;
        map.serialize_entry("result", &self.result.item)?;
        map.serialize_entry("count", &self.result.count)?;

        map.end()
    }
}

impl<'de> Deserialize<'de> for StonecuttingRecipe {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        deserializer.deserialize_map(StonecuttingRecipeVisitor)
    }
}


struct StonecuttingRecipeVisitor;

impl<'de> Visitor<'de> for StonecuttingRecipeVisitor {
    type Value = StonecuttingRecipe;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a stonecutting recipe")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where A: serde::de::MapAccess<'de> {
        map_visitor!(
            map,
            (group, "group", String),
            (ingredient, "ingredient", Ingredient),
            (result, "result", UnlocalizedName),
            (count, "count", u8)
        );

        missing_field_error!(ingredient, "ingredient", result, "result", count, "count");

        Ok(StonecuttingRecipe {
            group,
            input: ingredient,
            result: RecipeOutput {
                item: result,
                count,
            },
        })
    }
}

#[test]
fn stonecutting_ser_test() {
    use crate::data::recipe::{ingredient::Ingredient, VanillaRecipeType};

    let recipe = VanillaRecipeType::StonecuttingRecipe(StonecuttingRecipe {
        group: None,
        input: Ingredient::Item(UnlocalizedName::minecraft("stone")),
        result: RecipeOutput {
            item: UnlocalizedName::minecraft("stone_brick_slab"),
            count: 2,
        },
    });

    let serialized = serde_json::to_string(&recipe).unwrap();

    assert_eq!(
        &serialized,
        r#"{"type":"minecraft:stonecutting","ingredient":{"item":"minecraft:stone"},"result":"minecraft:stone_brick_slab","count":2}"#
    )
}

#[test]
fn stonecutting_de_test() {
    use crate::data::recipe::{ingredient::Ingredient, VanillaRecipeType};

    let input = r#"{
        "type": "minecraft:stonecutting",
        "ingredient": {
            "item": "minecraft:stone"
        },
        "result": "minecraft:stone_brick_slab",
        "count": 2
    }"#;

    let recipe: VanillaRecipeType = serde_json::from_str(input).unwrap();

    assert_eq!(
        recipe,
        VanillaRecipeType::StonecuttingRecipe(StonecuttingRecipe {
            group: None,
            input: Ingredient::Item(UnlocalizedName::minecraft("stone")),
            result: RecipeOutput {
                item: UnlocalizedName::minecraft("stone_brick_slab"),
                count: 2
            }
        })
    );
}

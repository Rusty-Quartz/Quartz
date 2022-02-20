use std::collections::BTreeMap;

use serde::{de::Visitor, ser::SerializeSeq, Deserialize, Serialize};

use crate::data::recipe::{ingredient::Ingredient, RecipeOutput};

/// A shapeless crafting recipe
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct ShapelessCraftingRecipe {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(rename = "ingredients")]
    pub inputs: ShapelessIngredients,
    pub result: RecipeOutput,
}

/// The ingredients list of a shapeless recipe
#[derive(Debug, PartialEq, Eq)]
pub struct ShapelessIngredients(Box<[(Ingredient, u8)]>);

impl Serialize for ShapelessIngredients {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;

        for (ingr, count) in self.0.iter() {
            for _ in 0 .. *count {
                seq.serialize_element(ingr)?;
            }
        }

        seq.end()
    }
}

impl<'de> Deserialize<'de> for ShapelessIngredients {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        deserializer.deserialize_seq(ShapelessIngredientsVisitor)
    }
}

struct ShapelessIngredientsVisitor;

impl<'de> Visitor<'de> for ShapelessIngredientsVisitor {
    type Value = ShapelessIngredients;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("List of ingredients")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where A: serde::de::SeqAccess<'de> {
        let mut map = BTreeMap::new();

        while let Some(i) = seq.next_element::<Ingredient>()? {
            // Clippy's warning here just is not valid for this use case I think
            #[allow(clippy::map_entry)]
            if map.contains_key(&i) {
                *map.get_mut(&i).unwrap() += 1;
            } else {
                map.insert(i, 1);
            }
        }

        Ok(ShapelessIngredients(
            map.into_iter().collect::<Vec<_>>().into_boxed_slice(),
        ))
    }
}

#[test]
fn shapeless_ser_test() {
    use crate::data::recipe::{ingredient::Ingredient, RecipeOutput, VanillaRecipeType};
    use qdat::UnlocalizedName;

    let recipe = VanillaRecipeType::ShapelessRecipe(ShapelessCraftingRecipe {
        inputs: ShapelessIngredients(Box::new([
            (Ingredient::Item(UnlocalizedName::minecraft("sand")), 4),
            (Ingredient::Item(UnlocalizedName::minecraft("gravel")), 4),
            (Ingredient::Item(UnlocalizedName::minecraft("red_dye")), 1),
        ])),
        result: RecipeOutput {
            item: UnlocalizedName::minecraft("red_concrete_powder"),
            count: 8,
        },
        group: Some("concrete_powder".to_owned()),
    });

    let serialized = serde_json::to_string(&recipe).unwrap();

    assert_eq!(
        r#"{"type":"minecraft:crafting_shapeless","group":"concrete_powder","ingredients":[{"item":"minecraft:sand"},{"item":"minecraft:sand"},{"item":"minecraft:sand"},{"item":"minecraft:sand"},{"item":"minecraft:gravel"},{"item":"minecraft:gravel"},{"item":"minecraft:gravel"},{"item":"minecraft:gravel"},{"item":"minecraft:red_dye"}],"result":{"item":"minecraft:red_concrete_powder","count":8}}"#,
        &serialized
    )
}

#[test]
fn shapeless_de_test() {
    use crate::data::recipe::{ingredient::Ingredient, RecipeOutput, VanillaRecipeType};
    use qdat::UnlocalizedName;

    let input = r#"{
        "type": "minecraft:crafting_shapeless",
        "group": "concrete_powder",
        "ingredients": [
          {"item": "minecraft:red_dye"},
          {"item": "minecraft:sand"},
          {"item": "minecraft:sand"},
          {"item": "minecraft:sand"},
          {"item": "minecraft:sand"},
          {"item": "minecraft:gravel"},
          {"item": "minecraft:gravel"},
          {"item": "minecraft:gravel"},
          {"item": "minecraft:gravel"}
        ],
        "result": {
          "item": "minecraft:red_concrete_powder",
          "count": 8
        }
      }"#;

    let recipe: VanillaRecipeType = serde_json::from_str(input).unwrap();

    assert_eq!(
        recipe,
        VanillaRecipeType::ShapelessRecipe(ShapelessCraftingRecipe {
            inputs: ShapelessIngredients(Box::new([
                (Ingredient::Item(UnlocalizedName::minecraft("gravel")), 4),
                (Ingredient::Item(UnlocalizedName::minecraft("red_dye")), 1),
                (Ingredient::Item(UnlocalizedName::minecraft("sand")), 4),
            ])),
            result: RecipeOutput {
                item: UnlocalizedName::minecraft("red_concrete_powder"),
                count: 8,
            },
            group: Some("concrete_powder".to_owned()),
        })
    )
}

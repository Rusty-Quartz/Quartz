use std::collections::{BTreeMap, HashMap};

use qdat::UnlocalizedName;
use serde::{de::Visitor, ser::SerializeMap, Deserialize, Serialize};

use crate::data::recipe::{ingredient::Ingredient, recipe::RecipeOutput};


/// A shaped crafting recipe
#[derive(PartialEq, Eq, Debug)]
pub struct ShapedCraftingRecipe {
    /// The pattern of the recipe
    /// # Note
    /// All the rows of the pattern must be the same width, with the width being determined by how many Some variants are included
    ///
    /// Both None and minecraft:air can represent an empty slot in the recipe
    /// The difference is that None is treated as nothing while air is still treated as a slot
    /// This means that air blocks a recipe from moving while None does not
    ///
    /// for example a recipe with this input
    /// ```json
    /// [[Some("minecraft:diamond"), Some("minecraft:diamond"), None],
    ///  [Some("minecraft:diamond"), Some("minecraft:stick"), None],
    ///  [Some("minecraft:air"),     Some("minecraft:stick"), None]]
    /// ```
    /// Would allow the recipe to be shifted to the right while
    /// ```json
    /// [[Some("minecraft:diamond"), Some("minecraft:diamond"), Some("minecraft:air")],
    ///  [Some("minecraft:diamond"), Some("minecraft:stick"), Some("minecraft:air")],
    ///  [Some("minecraft:air"),     Some("minecraft:stick"), Some("minecraft:air")]]
    /// ```
    /// Would not
    pub input: [[Option<Ingredient>; 3]; 3],
    pub group: Option<String>,
    pub result: RecipeOutput,
}

impl Serialize for ShapedCraftingRecipe {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        let mut item_ids = BTreeMap::new();
        let mut curr_key = 'a';
        let mut pattern = Vec::new();

        // Check to make sure the width of each row is the same
        // TODO: improve this check to account for [Some, None, Some]
        let width = self
            .input
            .iter()
            .map(|row| row.iter().filter(|i| i.is_some()).count())
            .collect::<Vec<_>>();

        // There are always 3 rows so we can also get this
        let first_width = width.first().unwrap();

        if width.iter().any(|w| w != first_width && *w != 0) {
            // I feel like this could be worded better
            return Err(serde::ser::Error::custom(
                "Shaped recipes must have all the rows with items be the same width",
            ));
        }

        for row in &self.input {
            let mut pat_row = String::new();
            for item in row.iter().flatten() {
                if !item_ids.contains_key(item) && !item.is_empty() {
                    item_ids.insert(item.clone(), curr_key);
                    curr_key = ((curr_key as u8) + 1) as char
                }
                pat_row.push(if item.is_empty() {
                    ' '
                } else {
                    *item_ids.get(item).unwrap()
                });
            }
            pattern.push(pat_row);
        }

        let mut map = serializer.serialize_map(Some(5))?;
        if self.group.is_some() {
            map.serialize_entry("group", &self.group)?;
        }
        map.serialize_entry("pattern", &pattern)?;
        map.serialize_entry(
            "key",
            &item_ids
                .iter()
                .map(|(k, v)| (v, k))
                .collect::<BTreeMap<_, _>>(),
        )?;
        map.serialize_entry("result", &self.result)?;

        map.end()
    }
}

impl<'de> Deserialize<'de> for ShapedCraftingRecipe {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        deserializer.deserialize_map(ShapedCraftingVisitor)
    }
}

struct ShapedCraftingVisitor;

impl<'de> Visitor<'de> for ShapedCraftingVisitor {
    type Value = ShapedCraftingRecipe;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("A shaped crafting recipe")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where A: serde::de::MapAccess<'de> {
        map_visitor!(
            map,
            (pattern, "pattern", Pattern),
            (item_map, "key", HashMap<char, Ingredient>),
            (result, "result", RecipeOutput),
            (group, "group", String)
        );

        missing_field_error!(pattern, "pattern", item_map, "key", result, "result");

        let mut input = [[None, None, None], [None, None, None], [None, None, None]];
        let mut e;

        for (i, row) in pattern.0.iter().enumerate() {
            e = 0;
            for c in row {
                if let Some(c) = c {
                    if *c == ' ' {
                        input[i][e] = Some(Ingredient::Item(UnlocalizedName::minecraft("air")))
                    } else {
                        input[i][e] = item_map.get(c).cloned()
                    }
                }
                e += 1;
            }
        }

        Ok(ShapedCraftingRecipe {
            input,
            result,
            group,
        })
    }
}

struct Pattern([[Option<char>; 3]; 3]);

impl<'de> Deserialize<'de> for Pattern {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        deserializer.deserialize_seq(PatternVisitor)
    }
}

struct PatternVisitor;

impl<'de> Visitor<'de> for PatternVisitor {
    type Value = Pattern;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("A shaped crafting pattern")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where A: serde::de::SeqAccess<'de> {
        let mut pattern = [[None; 3]; 3];
        let mut row = 0;
        let mut width = -1;

        while let Some(v) = seq.next_element::<String>()? {
            if row == 4 {
                return Err(serde::de::Error::custom(
                    "A shaped recipe can only be 3 rows long",
                ));
            }

            if width == -1 {
                // i8 should always be fine cause the max usable width should be 3
                width = v.len() as i8;
            }

            if v.len() as i8 != width {
                return Err(serde::de::Error::custom(
                    "Strings in shaped recipe patterns must all be the same length",
                ));
            }

            let mut chars = v.chars();
            pattern[row].iter_mut().for_each(|c| *c = chars.next());
            row += 1;
        }

        Ok(Pattern(pattern))
    }
}


#[test]
fn shaped_recipe_serialization() {
    use crate::data::recipe::{ingredient::Ingredient, VanillaRecipeType};
    let recipe = VanillaRecipeType::ShapedRecipe(ShapedCraftingRecipe {
        group: None,
        result: RecipeOutput {
            item: UnlocalizedName::minecraft("diamond_pickaxe"),
            count: 1,
        },
        input: [
            [
                Some(Ingredient::Item(UnlocalizedName::minecraft("diamond"))),
                Some(Ingredient::Item(UnlocalizedName::minecraft("diamond"))),
                Some(Ingredient::Item(UnlocalizedName::minecraft("diamond"))),
            ],
            [
                Some(Ingredient::Item(UnlocalizedName::minecraft("air"))),
                Some(Ingredient::Item(UnlocalizedName::minecraft("stick"))),
                Some(Ingredient::Item(UnlocalizedName::minecraft("air"))),
            ],
            [
                Some(Ingredient::Item(UnlocalizedName::minecraft("air"))),
                Some(Ingredient::Item(UnlocalizedName::minecraft("stick"))),
                Some(Ingredient::Item(UnlocalizedName::minecraft("air"))),
            ],
        ],
    });

    let str = serde_json::to_string(&recipe).unwrap();

    assert_eq!(
        r#"{"type":"minecraft:crafting_shaped","pattern":["aaa"," b "," b "],"key":{"a":{"item":"minecraft:diamond"},"b":{"item":"minecraft:stick"}},"result":{"item":"minecraft:diamond_pickaxe","count":1}}"#,
        &str
    )
}

#[test]
fn shaped_recipe_deserialization() {
    use crate::data::recipe::{ingredient::Ingredient, VanillaRecipeType};
    // mojang don't get mad that I just put your json here I just need test data ;-;
    let recipe = r##"{
        "type": "minecraft:crafting_shaped",
        "pattern": [
          "XXX",
          " # ",
          " # "
        ],
        "key": {
          "#": {
            "item": "minecraft:stick"
          },
          "X": {
            "item": "minecraft:diamond"
          }
        },
        "result": {
          "item": "minecraft:diamond_pickaxe"
        }
      }"##;

    let de: VanillaRecipeType = serde_json::from_str(recipe).unwrap();

    assert_eq!(
        de,
        VanillaRecipeType::ShapedRecipe(ShapedCraftingRecipe {
            group: None,
            result: RecipeOutput {
                item: UnlocalizedName::minecraft("diamond_pickaxe"),
                count: 1,
            },
            input: [
                [
                    Some(Ingredient::Item(UnlocalizedName::minecraft("diamond"))),
                    Some(Ingredient::Item(UnlocalizedName::minecraft("diamond"))),
                    Some(Ingredient::Item(UnlocalizedName::minecraft("diamond"))),
                ],
                [
                    Some(Ingredient::Item(UnlocalizedName::minecraft("air"))),
                    Some(Ingredient::Item(UnlocalizedName::minecraft("stick"))),
                    Some(Ingredient::Item(UnlocalizedName::minecraft("air"))),
                ],
                [
                    Some(Ingredient::Item(UnlocalizedName::minecraft("air"))),
                    Some(Ingredient::Item(UnlocalizedName::minecraft("stick"))),
                    Some(Ingredient::Item(UnlocalizedName::minecraft("air"))),
                ],
            ],
        })
    );
}

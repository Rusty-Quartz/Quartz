use std::{fmt::Debug, marker::PhantomData};

use qdat::UnlocalizedName;
use serde::{Deserialize, Serialize};

use crate::data::recipe::ingredient::Ingredient;

/// The generic format of a cooking recipe
///
/// The only thing that changes based on the cooking type is the amount of time it takes to cook<br>
/// Custom cooking recipes can be implemented by implementing [CookingRecipeType] on a struct
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct CookingRecipe<T: CookingRecipeType> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(rename = "ingredient")]
    pub input: Ingredient,
    pub result: UnlocalizedName,
    pub experience: f64,
    #[serde(default = "T::cook_time")]
    #[serde(rename = "cookingtime")]
    pub cook_time: u64,
    #[serde(skip)]
    pub _phatom: PhantomData<T>,
}

/// Represents the different type of cooking recipes by changing the cooking time
pub trait CookingRecipeType: Debug + PartialEq {
    /// Defines the default cooking time for this recipe type
    ///
    /// This is the only thing that differs between the cooking recipe types
    // NOTE: if they add const trait fns then change this to be const
    fn cook_time() -> u64;
}

#[macro_export]
macro_rules! smelting_type {
    ($($name: ident, $time: literal),*) => {
        $(
            #[doc = "To be used as the type parameter for [CookingRecipeType](quartz_datapack::data::recipe::cooking::CookingRecipeType)"]
            #[derive(Debug, PartialEq)]
            pub struct $name;
            impl CookingRecipeType for $name {
                fn cook_time() -> u64 {
                    $time
                }
            }
        )*
    };
}

smelting_type! {
    SmeltingRecipe, 200,
    BlastingRecipe, 100,
    SmokingRecipe, 100,
    // All vanilla campfire recipies have a cook time of 600 but the default is 100 for some reason
    CampfireRecipe, 100
}

#[test]
fn cooking_ser_test() {
    use crate::data::recipe::{ingredient::Ingredient, VanillaRecipeType};

    let recipe = VanillaRecipeType::SmeltingRecipe(CookingRecipe {
        group: Some("copper_ingot".to_owned()),
        input: Ingredient::Item(UnlocalizedName::minecraft("raw_copper")),
        result: UnlocalizedName::minecraft("copper_ingot"),
        cook_time: 200,
        experience: 0.7,
        _phatom: PhantomData,
    });

    let serialized = serde_json::to_string(&recipe).unwrap();

    assert_eq!(
        &serialized,
        r#"{"type":"minecraft:smelting","group":"copper_ingot","ingredient":{"item":"minecraft:raw_copper"},"result":"minecraft:copper_ingot","experience":0.7,"cookingtime":200}"#
    )
}

#[test]
fn cooking_de_test() {
    use crate::data::recipe::{ingredient::Ingredient, VanillaRecipeType};

    let input = r#"{
        "type": "minecraft:smelting",
        "group": "copper_ingot",
        "ingredient": {"item": "minecraft:raw_copper"},
        "result": "minecraft:copper_ingot",
        "experience": 0.7
    }"#;

    let recipe: VanillaRecipeType = serde_json::from_str(input).unwrap();

    assert_eq!(
        recipe,
        VanillaRecipeType::SmeltingRecipe(CookingRecipe {
            group: Some("copper_ingot".to_owned()),
            input: Ingredient::Item(UnlocalizedName::minecraft("raw_copper")),
            result: UnlocalizedName::minecraft("copper_ingot"),
            cook_time: 200,
            experience: 0.7,
            _phatom: PhantomData,
        })
    )
}

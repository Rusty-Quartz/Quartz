use qdat::UnlocalizedName;
use serde::{
    de::Visitor,
    ser::{SerializeMap, SerializeSeq},
    Deserialize,
    Serialize,
};

/// Represents an ingredient in a recipe
#[derive(PartialOrd, Ord, PartialEq, Eq, Clone, Debug)]
pub enum Ingredient {
    /// A single item
    Item(UnlocalizedName),
    /// A tag to use as the item provider
    Tag(UnlocalizedName),
    /// A list of ingredients to use as an ingredient provider
    /// # Note
    ///Do not have nested Ingredient::Lists
    List(Box<[Ingredient]>),
}

impl Ingredient {
    pub fn is_empty(&self) -> bool {
        match self {
            Ingredient::Item(i) => i == "minecraft:air",
            // As long as the UnlocalizedName is valid we need to resolve the tag
            Ingredient::Tag(_) => false,
            // the iter.any call can be expensive if the array is large
            // in practice lists should p much not be used cause tags exist sooooooo
            Ingredient::List(l) => l.len() == 0 || l.iter().any(|i| i.is_empty()),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct ItemJSON {
    pub item: String,
}
#[derive(Serialize, Deserialize)]
struct TagJSON {
    pub tag: String,
}
impl Serialize for Ingredient {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        match self {
            Ingredient::Item(i) => {
                let mut struc = serializer.serialize_map(Some(1))?;
                struc.serialize_entry("item", &i.to_string())?;
                struc.end()
            }
            Ingredient::Tag(t) => {
                let mut struc = serializer.serialize_map(Some(1))?;
                struc.serialize_entry("tag", &t.to_string())?;
                struc.end()
            }
            Ingredient::List(l) => {
                let mut arr = serializer.serialize_seq(Some(l.len()))?;
                for ingr in l.iter() {
                    match ingr {
                        Ingredient::Item(i) => {
                            arr.serialize_element(&ItemJSON {
                                item: i.to_string(),
                            })?;
                        }
                        Ingredient::Tag(t) => {
                            arr.serialize_element(&TagJSON { tag: t.to_string() })?;
                        }
                        Ingredient::List(_) =>
                            return Err(serde::ser::Error::custom(
                                "You can't have a list of ingredients inside another list of \
                                 ingredients",
                            )),
                    }
                }
                arr.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for Ingredient {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        deserializer.deserialize_any(IngredientVisitor)
    }
}

struct IngredientVisitor;

impl<'de> Visitor<'de> for IngredientVisitor {
    type Value = Ingredient;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("An ingredient")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where A: serde::de::MapAccess<'de> {
        let mut ingr = None;
        while let Some((key, value)) = map.next_entry::<&str, &str>()? {
            let uln = UnlocalizedName::from_str(value);
            if let Err(e) = uln {
                return Err(serde::de::Error::custom(format!(
                    "Invalid Identifier provided: {}",
                    e
                )));
            }
            // This error message kinda sucks
            if ingr.is_some() {
                return Err(serde::de::Error::custom(
                    "Multiple values specified in ingredient",
                ));
            }
            match key {
                "item" => ingr = Some(Ingredient::Item(uln.unwrap())),
                "tag" => ingr = Some(Ingredient::Tag(uln.unwrap())),
                _ => return Err(serde::de::Error::unknown_field(key, &["item", "tag"])),
            }
        }


        ingr.ok_or_else(|| serde::de::Error::missing_field("item or tag"))
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where A: serde::de::SeqAccess<'de> {
        let mut ingrs = Vec::new();

        while let Some(ingr) = seq.next_element::<Ingredient>()? {
            match ingr {
                Ingredient::List(_) =>
                    return Err(serde::de::Error::custom(
                        "Cannot have an ingredient list inside a list",
                    )),
                _ => {
                    ingrs.push(ingr);
                }
            }
        }

        Ok(Ingredient::List(ingrs.into_boxed_slice()))
    }
}

use qdat::UnlocalizedName;
use serde::{
    de::Visitor,
    ser::{SerializeMap, SerializeSeq},
    Deserialize,
    Serialize,
};

/// A [tag](https://minecraft.fandom.com/wiki/Tag)
pub struct Tag {
    pub def: TagDef,
    pub name: String,
}

impl Tag {
    pub fn replace(&self) -> bool {
        self.def.replace
    }

    pub fn values(&self) -> &Vec<TagEntry> {
        &self.def.values
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

/// The raw json definition of a tag
#[derive(Serialize, Debug, PartialEq)]
pub struct TagDef {
    pub replace: bool,
    pub values: Vec<TagEntry>,
}

impl<'de> Deserialize<'de> for TagDef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        deserializer.deserialize_any(TagDefVisitor)
    }
}

struct TagDefVisitor;

impl<'de> Visitor<'de> for TagDefVisitor {
    type Value = TagDef;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("A Tag")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where A: serde::de::MapAccess<'de> {
        map_visitor!(
            map,
            (replace, "replace", bool),
            (values, "values", Vec<TagEntry>)
        );
        missing_field_error!(replace, "replace", values, "values");

        Ok(TagDef { replace, values })
    }
}

/// One value of a tag
#[derive(Debug, PartialEq)]
pub enum TagEntry {
    /// A Namespace ID (aka UnlocalizedName), Ex: `minecraft:stone`
    NamespaceID(UnlocalizedName),
    /// A Tag entry, Ex: `#logs`
    ///
    /// When deserializing the '#' is stripped<br>
    /// When serializing the '#' is added
    Tag(String),
    /// A entry that can cause the tag to fail to load if lookup fails
    ///
    /// TagEntry in this context cannot be another FailableEntry
    /// this is enforced in the Serialize and Deserialze impls
    ///
    /// See [the minecraft wiki](https://minecraft.fandom.com/wiki/Tag#JSON_format) for more details
    FailableEntry(Box<TagEntry>, bool),
}

impl Serialize for TagEntry {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        match self {
            TagEntry::NamespaceID(n) => serializer.serialize_str(&n.to_string()),
            // If the tag already starts with a # we just serialize it
            TagEntry::Tag(t) if t.starts_with('#') => serializer.serialize_str(t),
            // Otherwise we add a # to the front
            TagEntry::Tag(t) => serializer.serialize_str(&format!("#{}", t)),
            TagEntry::FailableEntry(entry, required) => {
                let mut map = serializer.serialize_map(Some(2))?;
                match entry.as_ref() {
                    TagEntry::NamespaceID(n) => map.serialize_entry("value", n)?,
                    TagEntry::Tag(t) => map.serialize_entry("value", t)?,
                    TagEntry::FailableEntry(..) =>
                        return Err(serde::ser::Error::custom(
                            "TagEntry::EntryWithOptions cannot contain another EntryWithOptions",
                        )),
                };
                map.serialize_entry("required", required)?;
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for TagEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        deserializer.deserialize_any(TagEntryVisitor)
    }
}

struct TagEntryVisitor;

impl<'de> Visitor<'de> for TagEntryVisitor {
    type Value = TagEntry;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("A valid entry for a tag")
    }

    fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
    where E: serde::de::Error {
        if let Some(stripped) = v.strip_prefix('#') {
            // Cut off the # when deserializing to make it easier to use tag lookup
            Ok(TagEntry::Tag(stripped.to_owned()))
        } else {
            let uln = match UnlocalizedName::from_str(v) {
                Ok(u) => u,
                Err(e) =>
                    return Err(serde::de::Error::custom(format!(
                        "invalid identifier: {}",
                        e
                    ))),
            };
            Ok(TagEntry::NamespaceID(uln))
        }
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where A: serde::de::MapAccess<'de> {
        let mut value = None;
        let mut required = None;

        while let Some(key) = map.next_key()? {
            match key {
                "value" => {
                    if value.is_some() {
                        return Err(serde::de::Error::duplicate_field("value"));
                    }

                    value = Some(map.next_value::<TagEntryChecker>()?.0)
                }
                "required" => {
                    if required.is_some() {
                        return Err(serde::de::Error::duplicate_field("required"));
                    }

                    required = Some(map.next_value::<bool>()?)
                }
                _ => return Err(serde::de::Error::unknown_field(key, &["value", "required"])),
            }
        }

        if value.is_none() {
            return Err(serde::de::Error::missing_field("value"));
        }

        let value = value.unwrap();
        let value = if let Some(stripped) = value.strip_prefix('#') {
            TagEntry::Tag(stripped.to_owned())
        } else {
            let uln = match UnlocalizedName::from_str(value) {
                Ok(u) => u,
                Err(e) =>
                    return Err(serde::de::Error::custom(format!(
                        "invalid identifier: {}",
                        e
                    ))),
            };
            TagEntry::NamespaceID(uln)
        };

        Ok(TagEntry::FailableEntry(
            Box::new(value),
            required.unwrap_or(true),
        ))
    }
}

struct TagEntryChecker<'a>(&'a str);
struct TagEntryCheckerVisitor;

impl<'de> Deserialize<'de> for TagEntryChecker<'de> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        Ok(TagEntryChecker(
            deserializer.deserialize_any(TagEntryCheckerVisitor)?,
        ))
    }
}

impl<'de> Visitor<'de> for TagEntryCheckerVisitor {
    type Value = &'de str;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("A tag entry that is not FailableEntry")
    }

    fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
    where E: serde::de::Error {
        Ok(v)
    }

    fn visit_map<A>(self, _map: A) -> Result<Self::Value, A::Error>
    where A: serde::de::MapAccess<'de> {
        Err(serde::de::Error::custom(
            "A FailableEntry can not have another FailableEntry inside it",
        ))
    }
}

#[test]
fn tag_test() {
    let json = r#"{
        "replace": false,
        "values": [
            "minecraft:anvil",
            "minecraft:chipped_anvil",
            "minecraft:damaged_anvil"
        ]
    }"#;

    let tag_def: TagDef = serde_json::from_str(json).unwrap();

    assert_eq!(tag_def, TagDef {
        replace: false,
        values: vec![
            TagEntry::NamespaceID(UnlocalizedName::minecraft("anvil")),
            TagEntry::NamespaceID(UnlocalizedName::minecraft("chipped_anvil")),
            TagEntry::NamespaceID(UnlocalizedName::minecraft("damaged_anvil"))
        ]
    });
}

#[test]
fn tag_mixed_entry_test() {
    let json = r##"{
        "replace": false,
        "values": [
            "minecraft:anvil",
            "#logs",
            {
                "value": "minecraft:stone"
            }
        ]
    }"##;

    let tag_def: TagDef = serde_json::from_str(json).unwrap();

    assert_eq!(tag_def, TagDef {
        replace: false,
        values: vec![
            TagEntry::NamespaceID(UnlocalizedName::minecraft("anvil")),
            TagEntry::Tag("logs".to_owned()),
            TagEntry::FailableEntry(
                Box::new(TagEntry::NamespaceID(UnlocalizedName::minecraft("stone"))),
                true
            )
        ]
    });
}

#[test]
#[should_panic]
fn failable_test() {
    let json = r##"{
        "replace": false,
        "values": [
            {
                "value": {
                    "value": "minecraft:stone"
                }
            }
        ]
    }"##;

    let tag_def: TagDef = serde_json::from_str(json).unwrap();

    // We should fail before it reaches here
    assert_eq!(tag_def, TagDef {
        replace: false,
        values: vec![TagEntry::FailableEntry(
            Box::new(TagEntry::FailableEntry(
                Box::new(TagEntry::NamespaceID(UnlocalizedName::minecraft("stone"))),
                true
            )),
            true
        )]
    });
}

pub struct TagProvider(UnlocalizedName);

impl<'de> Deserialize<'de> for TagProvider {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        deserializer.deserialize_any(TagProviderVisitor)
    }
}

struct TagProviderVisitor;

impl<'de> Visitor<'de> for TagProviderVisitor {
    type Value = TagProvider;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "A tag prepended by a #")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where E: serde::de::Error {
        if let Some(v) = v.strip_prefix('#') {
            match UnlocalizedName::from_str(v) {
                Ok(uln) => Ok(TagProvider(uln)),
                Err(e) => Err(serde::de::Error::custom(format!(
                    "Invalid identifier found while parsing: {e}"
                ))),
            }
        } else {
            Err(serde::de::Error::custom(
                "A tag has to be prepended by a #".to_string(),
            ))
        }
    }
}

impl Serialize for TagProvider {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        serializer.serialize_str(&format!("#{}", self.0))
    }
}


/// A field that can be a reference to either an id, a tag, or a list of ids
///
/// Vanilla has the single id case be sugar for [id] internally but we store them seperately
#[derive(Hash, PartialEq, Eq, Debug)]
pub enum IdsOrTag {
    SingleTag(UnlocalizedName),
    SingleId(UnlocalizedName),
    IdList(Vec<UnlocalizedName>),
}

impl<'de> Deserialize<'de> for IdsOrTag {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        deserializer.deserialize_any(IdsOrTagVisitor)
    }
}

struct IdsOrTagVisitor;

impl<'de> Visitor<'de> for IdsOrTagVisitor {
    type Value = IdsOrTag;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "A reference to a tag or list of tags")
    }

    fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
    where E: serde::de::Error {
        if let Some(v) = v.strip_prefix('#') {
            match UnlocalizedName::from_str(v) {
                Ok(uln) => Ok(IdsOrTag::SingleTag(uln)),
                Err(e) => Err(serde::de::Error::custom(format!(
                    "Invalid identifier found while parsing: {e}"
                ))),
            }
        } else {
            match UnlocalizedName::from_str(v) {
                Ok(uln) => Ok(IdsOrTag::SingleId(uln)),
                Err(e) => Err(serde::de::Error::custom(format!(
                    "Invalid identifier found while parsing: {e}"
                ))),
            }
        }
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where A: serde::de::SeqAccess<'de> {
        let mut list = Vec::new();
        while let Ok(Some(v)) = seq.next_element::<&'de str>() {
            match UnlocalizedName::from_str(v) {
                Ok(uln) => list.push(uln),
                Err(e) =>
                    return Err(serde::de::Error::custom(format!(
                        "Invalid identifier found while parsing: {e}"
                    ))),
            }
        }
        Ok(IdsOrTag::IdList(list))
    }
}

impl Serialize for IdsOrTag {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        match self {
            IdsOrTag::SingleTag(uln) => serializer.serialize_str(&format!("#{uln}")),
            IdsOrTag::SingleId(uln) => serializer.serialize_str(uln.repr()),
            IdsOrTag::IdList(ulns) => {
                let mut seq = serializer.serialize_seq(Some(ulns.len()))?;
                for uln in ulns {
                    seq.serialize_element(uln.repr())?
                }
                seq.end()
            }
        }
    }
}

#[test]
fn ids_or_list_test() {
    use serde_json::{from_str, to_string_pretty};
    let json = r###"
        {
            "name": "hello",
            "block": "#logs",
            "block2": ["logs", "logs2"],
            "block3": "moss_replacable"
        }
    "###;

    #[derive(Serialize, Deserialize)]
    struct TestStruct {
        name: String,
        block: IdsOrTag,
        block2: IdsOrTag,
        block3: IdsOrTag,
    }

    let parsed = from_str::<TestStruct>(json);

    assert!(parsed.is_ok());

    let parsed = parsed.unwrap();

    assert_eq!(
        parsed.block,
        IdsOrTag::SingleTag(UnlocalizedName::minecraft("logs"))
    );

    assert_eq!(
        parsed.block2,
        IdsOrTag::IdList(vec![
            UnlocalizedName::minecraft("logs"),
            UnlocalizedName::minecraft("logs2")
        ])
    );

    assert_eq!(
        parsed.block3,
        IdsOrTag::SingleTag(UnlocalizedName::minecraft("moss_replacable"))
    );

    let str = to_string_pretty(&parsed);

    assert!(str.is_ok());

    let str = str.unwrap();

    assert_eq!(
        r###"{
  "name": "hello",
  "block": "logs",
  "block2": [
    "logs",
    "logs2"
  ],
  "block3": "moss_replacable"
}"###,
        str
    )
}

use std::collections::HashMap;
use std::convert::{AsMut, AsRef, TryFrom, TryInto};
use std::fmt;
use std::ops::{Index, IndexMut};
use std::option::NoneError;
use std::str::FromStr;
use chat::{
    Component,
    TextComponentBuilder,
    color::PredefinedColor,
    component::{ToComponent, ToComponentParts}
};
use crate::NbtRepr;
use crate::snbt::SnbtParser;

/// The generic NBT tag type, containing all supported tag variants which wrap around a corresponding rust type.
#[derive(Clone)]
pub enum NbtTag {
    /// A signed, one-byte integer.
    Byte(i8),
    /// A signed, two-byte integer.
    Short(i16),
    /// A signed, four-byte integer.
    Int(i32),
    /// A signed, eight-byte integer.
    Long(i64),
    /// A 32-bit floating point value.
    Float(f32),
    /// A 64-bit floating point value.
    Double(f64),
    /// An array (vec) of signed, one-byte integers.
    ByteArray(Vec<i8>),
    /// A string with a modified UTF-8 encoding to mirror Java's system
    StringModUtf8(String),
    /// An NBT tag list.
    List(NbtList),
    /// An NBT tag compound.
    Compound(NbtCompound),
    /// An array (vec) of signed, four-byte integers.
    IntArray(Vec<i32>),
    /// An array (vec) of signed, eight-byte integers.
    LongArray(Vec<i64>)
}

impl NbtTag {
    /// Returns the single character denoting this tag's type, or an empty string if this tag type has
    /// no type specifier.
    /// 
    /// # Examples
    /// 
    /// ```
    /// assert_eq!(NbtTag::Long(10).type_specifier(), "L");
    /// assert_eq!(NbtTag::StringModUtf8(String::new()).type_specifier(), "");
    /// 
    /// // Note that while integers do not require a type specifier, this method will still return "I"
    /// assert_eq!(NbtTag::Int(-10).type_specifier(), "I");
    /// ```
    pub fn type_specifier(&self) -> &str {
        match self {
            NbtTag::Byte(_) => "B",
            NbtTag::Short(_) => "S",
            NbtTag::Int(_) => "I",
            NbtTag::Long(_) => "L",
            NbtTag::Float(_) => "F",
            NbtTag::Double(_) => "D",
            NbtTag::ByteArray(_) => "B",
            NbtTag::IntArray(_) => "I",
            NbtTag::LongArray(_) => "L",
            _ => ""
        }
    }

    /// Returns the name of each tag type
    pub fn type_string(&self) -> &str {
        match self {
            NbtTag::Byte(_) => "Byte",
            NbtTag::Short(_) => "Short",
            NbtTag::Int(_) => "Int",
            NbtTag::Long(_) => "Long",
            NbtTag::Float(_) => "Float",
            NbtTag::Double(_) => "Double",
            NbtTag::StringModUtf8(_) => "String",
            NbtTag::ByteArray(_) => "Byte Array",
            NbtTag::IntArray(_) => "Int Array",
            NbtTag::LongArray(_) => "Long Array",
            NbtTag::Compound(_) => "Compound",
            NbtTag::List(_) => "List"
        }
    }

    /// Converts this NBT tag into a valid, parsable SNBT string with no extraneous spacing. This method should
    /// not be used to generate user-facing text, rather `to_component` should be used instead.
    /// 
    /// # Examples
    /// 
    /// Simple primitive conversion:
    /// 
    /// ```
    /// assert_eq!(NbtTag::Byte(5).to_snbt(), "5B");
    /// assert_eq!(NbtTag::StringModUtf8("\"Quoted text\""), "'\"Quoted text\"'");
    /// ```
    /// 
    /// More complex tag conversion:
    /// 
    /// ```
    /// let mut compound = NbtCompound::new();
    /// compound.set_long_array("foo".to_owned(), vec![-1_i64, -3_i64, -5_i64]);
    /// assert_eq!(NbtTag::Compound(compound).to_snbt(), "{foo:[L;-1,-3,-5]}");
    /// ```
    pub fn to_snbt(&self) -> String {
        macro_rules! list_to_string {
            ($list:expr) => {
                format!("[{};{}]", self.type_specifier(), $list.iter().map(ToString::to_string).collect::<Vec<String>>().join(","))
            }
        }

        match self {
            NbtTag::Byte(value) => format!("{}{}", value, self.type_specifier()),
            NbtTag::Short(value) => format!("{}{}", value, self.type_specifier()),
            NbtTag::Int(value) => format!("{}", value),
            NbtTag::Long(value) => format!("{}{}", value, self.type_specifier()),
            NbtTag::Float(value) => format!("{}{}", value, self.type_specifier()),
            NbtTag::Double(value) => format!("{}{}", value, self.type_specifier()),
            NbtTag::ByteArray(value) => list_to_string!(value),
            NbtTag::StringModUtf8(value) => Self::string_to_snbt(value),
            NbtTag::List(value) => value.to_snbt(),
            NbtTag::Compound(value) => value.to_snbt(),
            NbtTag::IntArray(value) => list_to_string!(value),
            NbtTag::LongArray(value) => list_to_string!(value)
        }
    }

    /// Returns whether or not the given string needs to be quoted due to non-alphanumeric or otherwise 
    /// non-standard characters.
    fn should_quote(string: &str) -> bool {
        for ch in string.chars() {
            if (ch < '0' || ch > '9') && (ch < 'A' || ch > 'Z') && (ch < 'a' || ch > 'z') && ch != '_' && ch != '-' && ch != '.' {
                return true;
            }
        }

        false
    }

    /// Wraps the given string in quotes and escapes any quotes contained in the original string.
    fn string_to_snbt(string: &str) -> String {
        // Determine the best option for the surrounding quotes to minimize escape sequences
        let surrounding: char;
        if string.contains("\"") {
            surrounding = '\'';
        } else {
            surrounding = '"';
        }

        let mut snbt_string = String::with_capacity(2 + string.len());
        snbt_string.push(surrounding);

        // Construct the string accounting for escape sequences
        for ch in string.chars() {
            if ch == surrounding || ch == '\\' {
                snbt_string.push('\\');
            }
            snbt_string.push(ch);
        }

        snbt_string.push(surrounding);
        snbt_string
    }
}

impl ToComponentParts for NbtTag {
    fn to_component_parts(&self) -> Vec<Component> {
        macro_rules! primitive_to_component {
            ($value:expr) => {
                TextComponentBuilder::empty()
                    .add()
                    .text(format!("{}", $value))
                    .predef_color(PredefinedColor::Gold)
                    .add()
                    .text(self.type_specifier().to_owned())
                    .predef_color(PredefinedColor::Red)
                    .build_children()
            };
        }

        macro_rules! list_to_component {
            ($list:expr) => {{
                // Handle the empty case
                if $list.is_empty() {
                    return TextComponentBuilder::empty()
                        .add()
                        .text("[".to_owned())
                        .add()
                        .text(self.type_specifier().to_owned())
                        .predef_color(PredefinedColor::Red)
                        .add()
                        .text(";]".to_owned())
                        .build_children();
                }
        
                // 1+ elements
        
                let mut builder = TextComponentBuilder::empty()
                    .add()
                    .text("[".to_owned())
                    .add()
                    .text(self.type_specifier().to_owned())
                    .predef_color(PredefinedColor::Red)
                    .add()
                    .text("; ".to_owned())
                    .add()
                    .text(format!("{}", $list[0]))
                    .predef_color(PredefinedColor::Gold);
                
                for element in $list.iter().skip(1) {
                    builder = builder.add()
                        .text(", ".to_owned())
                        .add()
                        .text(format!("{}", element))
                        .predef_color(PredefinedColor::Gold);
                }
        
                builder.add().text("]".to_owned()).build_children()
            }};
        }

        match self {
            NbtTag::Byte(value) => primitive_to_component!(value),
            NbtTag::Short(value) => primitive_to_component!(value),
            NbtTag::Int(value) => vec![Component::colored(format!("{}", value), PredefinedColor::Gold)],
            NbtTag::Long(value) => primitive_to_component!(value),
            NbtTag::Float(value) => primitive_to_component!(value),
            NbtTag::Double(value) => primitive_to_component!(value),
            NbtTag::ByteArray(value) => list_to_component!(value),
            NbtTag::StringModUtf8(value) => {
                // Determine the best option for the surrounding quotes to minimize escape sequences
                let surrounding: char;
                if value.contains("\"") {
                    surrounding = '\'';
                } else {
                    surrounding = '"';
                }

                let mut snbt_string = String::with_capacity(value.len());

                // Construct the string accounting for escape sequences
                for ch in value.chars() {
                    if ch == surrounding || ch == '\\' {
                        snbt_string.push('\\');
                    }
                    snbt_string.push(ch);
                }

                TextComponentBuilder::empty()
                    .add()
                    .text(surrounding.to_string())
                    .add()
                    .text(snbt_string)
                    .predef_color(PredefinedColor::Green)
                    .add()
                    .text(surrounding.to_string())
                    .build_children()
            },
            NbtTag::List(value) => value.to_component_parts(),
            NbtTag::Compound(value) => value.to_component_parts(),
            NbtTag::IntArray(value) => list_to_component!(value),
            NbtTag::LongArray(value) => list_to_component!(value)
        }
    }
}

impl ToComponent for NbtTag { }

// Display the tag in a user-friendly form
impl fmt::Display for NbtTag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.to_component().fmt(f)
    }
}

// Implement the from trait for all the tag's internal types
macro_rules! tag_from {
    ($($type:ty, $tag:ident);*) => {
        $(
            impl From<$type> for NbtTag {
                fn from(value: $type) -> NbtTag {
                    NbtTag::$tag(value)
                }
            }
        )*
    };
}

tag_from!(
    i8, Byte;
    i16, Short;
    i32, Int;
    i64, Long;
    f32, Float;
    f64, Double;
    Vec<i8>, ByteArray;
    String, StringModUtf8;
    NbtList, List;
    NbtCompound, Compound;
    Vec<i32>, IntArray;
    Vec<i64>, LongArray
);

impl From<&str> for NbtTag {
    fn from(value: &str) -> NbtTag {
        NbtTag::StringModUtf8(value.to_owned())
    }
}

impl From<bool> for NbtTag {
    fn from(value: bool) -> NbtTag {
        NbtTag::Byte(if value { 1 } else { 0 })
    }
}

impl<T: NbtRepr> From<T> for NbtTag {
    #[inline]
    fn from(x: T) -> Self {
        NbtTag::Compound(x.to_nbt())
    }
}

macro_rules! prim_from_tag {
    ($($type:ty, $tag:ident);*) => {
        $(
            impl TryFrom<&NbtTag> for $type {
                type Error = NoneError;

                fn try_from(tag: &NbtTag) -> Result<Self, Self::Error> {
                    if let NbtTag::$tag(value) = tag {
                        Ok(*value)
                    } else {
                        Err(NoneError)
                    }
                }
            }
        )*
    };
}

prim_from_tag!(
    i8, Byte;
    i16, Short;
    i32, Int;
    i64, Long;
    f32, Float;
    f64, Double
);

impl TryFrom<&NbtTag> for bool {
    type Error = NoneError;

    fn try_from(tag: &NbtTag) -> Result<Self, Self::Error> {
        match tag {
            NbtTag::Byte(value) => Ok(*value != 0),
            NbtTag::Short(value) => Ok(*value != 0),
            NbtTag::Int(value) => Ok(*value != 0),
            NbtTag::Long(value) => Ok(*value != 0),
            _ => Err(NoneError)
        }
    }
}

macro_rules! ref_from_tag {
    ($($type:ty, $tag:ident);*) => {
        $(
            impl<'a> TryFrom<&'a NbtTag> for &'a $type {
                type Error = NoneError;

                fn try_from(tag: &'a NbtTag) -> Result<Self, Self::Error> {
                    if let NbtTag::$tag(value) = tag {
                        Ok(value)
                    } else {
                        Err(NoneError)
                    }
                }
            }

            impl<'a> TryFrom<&'a mut NbtTag> for &'a mut $type {
                type Error = NoneError;

                fn try_from(tag: &'a mut NbtTag) -> Result<Self, Self::Error> {
                    if let NbtTag::$tag(value) = tag {
                        Ok(value)
                    } else {
                        Err(NoneError)
                    }
                }
            }
        )*
    };
}

ref_from_tag!(
    i8, Byte;
    i16, Short;
    i32, Int;
    i64, Long;
    f32, Float;
    f64, Double;
    Vec<i8>, ByteArray;
    [i8], ByteArray;
    String, StringModUtf8;
    str, StringModUtf8;
    NbtList, List;
    NbtCompound, Compound;
    Vec<i32>, IntArray;
    [i32], IntArray;
    Vec<i64>, LongArray;
    [i64], LongArray
);

/// The NBT tag list type which is essentially just a wrapper for a vec of NBT tags.
#[repr(transparent)]
#[derive(Clone)]
pub struct NbtList(Vec<NbtTag>);

impl NbtList {
    /// Returns a new NBT tag list with an empty internal vec.
    pub fn new() -> Self {
        NbtList(Vec::new())
    }

    /// Returns a new NBT tag list with the given initial capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        NbtList(Vec::with_capacity(capacity))
    }

    /// Clones the data in the given list and converts it into an [`NbtList`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use nbt::NbtList;
    /// let list: Vec<i32> = vec![1, 2, 3];
    /// let nbt_list = NbtList::clone_from(&list);
    /// assert_eq!(nbt_list.iter_map::<_, i32>().flatten().collect::<Vec<i32>>(), list);
    /// ```
    ///
    /// [`NbtList`]: crate::tag::NbtList
    pub fn clone_from<'a, T, L>(list: &'a L) -> Self
    where
        T: Clone + Into<NbtTag> + 'a,
        &'a L: IntoIterator<Item = &'a T>
    {
        NbtList(list.into_iter().map(|x| x.clone().into()).collect())
    }

    /// Creates an [`NbtList`] of [`NbtCompound`]s by mapping each element in the given list to its
    /// NBT representation.
    ///
    /// [`NbtCompound`]: crate::tag::NbtCompound
    /// [`NbtList`]: crate::tag::NbtList
    pub fn clone_repr_from<'a, T, L>(list: &'a L) -> Self
    where
        T: NbtRepr + 'a,
        &'a L: IntoIterator<Item = &'a T>
    {
        NbtList(list.into_iter().map(|x| x.to_nbt().into()).collect())
    }

    pub fn iter_map<'a, E, T: TryFrom<&'a NbtTag, Error = E>>(&'a self) -> impl Iterator<Item = Result<T, E>> + 'a {
        self.0.iter().map(|tag| T::try_from(tag))
    }

    pub fn iter_mut_map<'a, E, T: TryFrom<&'a mut NbtTag, Error = E>>(&'a mut self) -> impl Iterator<Item = Result<T, E>> + 'a {
        self.0.iter_mut().map(|tag| T::try_from(tag))
    }

    pub fn iter_into_repr<T>(&self) -> impl Iterator<Item = Result<T, T::Error>> + '_
    where
        T: NbtRepr,
        T::Error: From<NoneError>
    {
        self.0.iter().map(|tag| T::from_nbt(tag.try_into()?))
    }

    pub fn clone_into<'a, T, L>(&'a self, list: &mut L)
    where
        T: Clone + 'a,
        &'a T: TryFrom<&'a NbtTag>,
        L: Extend<T>
    {
        list.extend(self.0.iter().flat_map(|tag| TryInto::<&T>::try_into(tag).ok().map(|x| x.clone())));
    }

    pub fn clone_repr_into<'a, T, L>(&'a self, list: &mut L)
    where
        T: NbtRepr,
        T::Error: From<NoneError>,
        L: Extend<T>
    {
        list.extend(self.0.iter().flat_map(|tag| T::from_nbt(tag.try_into()?)));
    }

    /// Converts this tag list to a valid SNBT string.
    pub fn to_snbt(&self) -> String {
        let mut snbt_list = String::with_capacity(2 + 8 * self.len());
        snbt_list.push('[');
        snbt_list.push_str(&self.as_ref().iter().map(|tag| tag.to_snbt()).collect::<Vec<String>>().join(","));
        snbt_list.push(']');
        snbt_list
    }

    /// Returns the length of this list.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if this tag list has a length of zero, false otherwise.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the value of the tag at the given index, or `None` if the index is out of bounds. This method
    /// should be used for obtaining primitives and shared references to lists and compounds.
    pub fn get<'a, T: TryFrom<&'a NbtTag>>(&'a self, index: usize) -> Option<T> {
        T::try_from(self.0.get(index)?).ok()
    }

    /// Returns a mutable reference to the tag at the given index, or `None` if the index is out of bounds. This
    /// method should be used for obtaining mutable references to lists and compounds.
    pub fn get_mut<'a, T: TryFrom<&'a mut NbtTag>>(&'a mut self, index: usize) -> Option<T> {
        T::try_from(self.0.get_mut(index)?).ok()
    }

    /// Pushes the given value to the back of the list after wrapping it in an `NbtTag`.
    pub fn add<T: Into<NbtTag>>(&mut self, value: T) {
        self.0.push(value.into());
    }
}

impl<T: Into<NbtTag>> From<Vec<T>> for NbtList {
    fn from(list: Vec<T>) -> Self {
        NbtList(list.into_iter().map(|x| x.into()).collect())
    }
}

impl AsRef<Vec<NbtTag>> for NbtList {
    #[inline]
    fn as_ref(&self) -> &Vec<NbtTag> {
        &self.0
    }
}

impl AsMut<Vec<NbtTag>> for NbtList {
    #[inline]
    fn as_mut(&mut self) -> &mut Vec<NbtTag> {
        &mut self.0
    }
}

impl ToComponentParts for NbtList {
    fn to_component_parts(&self) -> Vec<Component> {
        if self.is_empty() {
            return vec![Component::text("[]".to_owned())];
        }

        let mut components = Vec::with_capacity(2 + 3 * self.len());
        components.push(Component::text("[".to_owned()));
        components.extend(self[0].to_component_parts());

        for tag in self.as_ref().iter().skip(1) {
            components.push(Component::text(", ".to_owned()));
            components.extend(tag.to_component_parts());
        }

        components.push(Component::text("]".to_owned()));
        components
    }
}

impl ToComponent for NbtList { }

impl fmt::Display for NbtList {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.to_component().fmt(f)
    }
}

impl Index<usize> for NbtList {
    type Output = NbtTag;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl IndexMut<usize> for NbtList {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

/// The NBT tag compound type which is essentially just a wrapper for a hash map of string keys
/// to tag values.
#[repr(transparent)]
#[derive(Clone)]
pub struct NbtCompound(HashMap<String, NbtTag>);

impl NbtCompound {
    /// Returns a new NBT tag compound with an empty internal hash map.
    pub fn new() -> Self {
        NbtCompound(HashMap::new())
    }

    /// Returns a new NBT tag compound with the given initial capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        NbtCompound(HashMap::with_capacity(capacity))
    }

    pub fn clone_from<'a, K, V, M>(map: &'a M) -> Self
    where
        K: ToString + 'a,
        V: Clone + Into<NbtTag> + 'a,
        &'a M: IntoIterator<Item = (&'a K, &'a V)>
    {
        NbtCompound(map.into_iter().map(|(key, value)| (key.to_string(), value.clone().into())).collect())
    }

    pub fn clone_repr_from<'a, K, V, M>(map: &'a M) -> Self
    where
        K: ToString + 'a,
        V: NbtRepr + 'a,
        &'a M: IntoIterator<Item = (&'a K, &'a V)>
    {
        NbtCompound(map.into_iter().map(|(key, value)| (key.to_string(), value.to_nbt().into())).collect())
    }

    pub fn iter_map<'a, E, T: TryFrom<&'a NbtTag, Error = E>>(&'a self) -> impl Iterator<Item = (&'a String, Result<T, E>)> + 'a {
        self.0.iter().map(|(key, tag)| (key, T::try_from(tag)))
    }

    pub fn iter_mut_map<'a, E, T: TryFrom<&'a mut NbtTag, Error = E>>(&'a mut self) -> impl Iterator<Item = (&'a String, Result<T, E>)> + 'a {
        self.0.iter_mut().map(|(key, tag)| (key, T::try_from(tag)))
    }

    pub fn iter_into_repr<T>(&self) -> impl Iterator<Item = (&'_ String, Result<T, T::Error>)> + '_
    where
        T: NbtRepr,
        T::Error: From<NoneError>
    {
        self.0.iter().map(|(key, tag)| {
            match TryInto::<&NbtCompound>::try_into(tag) {
                Ok(nbt) => (key, T::from_nbt(nbt)),
                Err(_) => (key, Err(T::Error::from(NoneError)))
            }
        })
    }

    pub fn clone_into_map<'a, K, V, M>(&'a self, map: &mut M)
    where
        K: FromStr,
        V: Clone + 'a,
        &'a V: TryFrom<&'a NbtTag>,
        M: Extend<(K, V)>
    {
        map.extend(self.0.iter().flat_map(|(key, tag)| {
            Some((K::from_str(key).ok()?, TryInto::<&V>::try_into(tag).ok()?.clone()))
        }));
    }

    pub fn clone_repr_into_map<'a, K, V, M>(&'a self, map: &mut M)
    where
        K: FromStr,
        V: NbtRepr,
        M: Extend<(K, V)>
    {
        map.extend(self.0.iter().flat_map(|(key, tag)| {
            Some((K::from_str(key).ok()?, V::from_nbt(tag.try_into()?).ok()?))
        }));
    }

    #[inline]
    pub fn clone_into<T: NbtRepr>(&self) -> Result<T, T::Error> {
        T::from_nbt(self)
    }

    /// Converts this tag compound into a valid SNBT string.
    pub fn to_snbt(&self) -> String {
        let mut snbt_compound = String::with_capacity(2 + 16 * self.len());
        snbt_compound.push('{');
        snbt_compound.push_str(
            &self.as_ref().iter()
                .map(|(key, tag)| {
                    if NbtTag::should_quote(key) {
                        format!("{}:{}", NbtTag::string_to_snbt(key), tag.to_snbt())
                    } else {
                        format!("{}:{}", key, tag.to_snbt())
                    }
                })
                .collect::<Vec<String>>()
                .join(",")
        );
        snbt_compound.push('}');
        snbt_compound
    }

    /// Returns the number of tags in this compound.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if the length of this compound is zero, false otherwise.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the value of the tag with the given name, or `None` if no tag could be found with the given name.
    /// This method should be used to obtain primitives as well as shared references to lists and compounds.
    pub fn get<'a, T: TryFrom<&'a NbtTag>>(&'a self, name: &str) -> Option<T> {
        T::try_from(self.0.get(name)?).ok()
    }

    /// Returns the value of the tag with the given name, or `None` if no tag could be found with the given name.
    /// This method should be used to obtain mutable references to lists and compounds.
    pub fn get_mut<'a, T: TryFrom<&'a mut NbtTag>>(&'a mut self, name: &str) -> Option<T> {
        T::try_from(self.0.get_mut(name)?).ok()
    }

    /// Returns whether or not this compound has a tag with the given name.
    #[inline]
    pub fn has(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    /// Adds the given value to this compound with the given name after wrapping that value in an `NbtTag`.
    pub fn set<T: Into<NbtTag>>(&mut self, name: String, value: T) {
        self.0.insert(name, value.into());
    }

    /// Parses a nbt compound from snbt
    /// # Example
    /// ```
    /// let tag = NbtCompound::from_snbt(r#"{string:Stuff, list:[I;1,2,3,4,5]}"#).unwrap();
    /// assert_eq!(tag.get_string("string"), "Stuff");
    /// assert_eq!(tag.get_int_array("list"), vec![1,2,3,4,5]);
    /// ```
    pub fn from_snbt(input: &str) -> Result<Self, String> {
        let input = input.to_owned();
        let mut parser = SnbtParser::new(&input, 0);

        parser.parse()
    }
}

impl AsRef<HashMap<String, NbtTag>> for NbtCompound {
    #[inline]
    fn as_ref(&self) -> &HashMap<String, NbtTag> {
        &self.0
    }
}

impl AsMut<HashMap<String, NbtTag>> for NbtCompound {
    #[inline]
    fn as_mut(&mut self) -> &mut HashMap<String, NbtTag> {
        &mut self.0
    }
}

impl ToComponentParts for NbtCompound {
    fn to_component_parts(&self) -> Vec<Component> {
        if self.is_empty() {
            return vec![Component::text("{}".to_owned())];
        }

        let mut components = Vec::with_capacity(2 + 3 * self.len());

        // Grab the elements and push the first one
        let elements = self.as_ref().iter().collect::<Vec<(&String, &NbtTag)>>();
        components.push(Component::text("{".to_owned()));

        // Push the rest of the elements
        for element in elements.iter() {
            // The key contains special characters and needs to be quoted/escaped
            if NbtTag::should_quote(element.0) {
                // Convert the key to an SNBT string
                let snbt_key = NbtTag::string_to_snbt(element.0);
                // Get the quote type used
                let quote = snbt_key.as_bytes()[0] as char;

                if components.len() > 1 {
                    components.push(Component::text(format!(", {}", quote)));
                }
                components.push(Component::colored(snbt_key[1..snbt_key.len() - 1].to_owned(), PredefinedColor::Aqua));
                components.push(Component::text(format!("{}: ", quote)));
            }
            // They key can be pushed as-is
            else {
                if components.len() > 1 {
                    components.push(Component::text(", ".to_owned()));
                }
                components.push(Component::colored(element.0.to_owned(), PredefinedColor::Aqua));
                components.push(Component::text(": ".to_owned()));
            }

            // Add the tag's components
            components.extend(element.1.to_component_parts());
        }

        components.push(Component::text("}".to_owned()));
        components
    }
}

impl ToComponent for NbtCompound { }

// Display the compound as valid SNBT format
impl fmt::Display for NbtCompound {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.to_component().fmt(f)
    }
}
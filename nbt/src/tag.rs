use crate::{
    snbt::{self, ParserError},
    NbtRepr,
    NbtReprError,
    NbtStructureError,
};
use std::{
    borrow::Borrow,
    collections::HashMap,
    convert::{AsMut, AsRef, TryFrom, TryInto},
    fmt::{self, Debug, Display, Formatter},
    hash::Hash,
    ops::{Index, IndexMut},
};

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
    LongArray(Vec<i64>),
}

impl NbtTag {
    /// Returns the single character denoting this tag's type, or an empty string if this tag type has
    /// no type specifier.
    ///
    /// # Examples
    ///
    /// ```
    /// # use nbt::NbtTag;
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
            _ => "",
        }
    }

    /// Returns the name of each tag type
    ///
    /// # Examples
    ///
    /// ```
    /// # use nbt::NbtTag;
    /// assert_eq!(NbtTag::Float(0.0f32).type_string(), "Float");
    /// assert_eq!(NbtTag::LongArray(Vec::new()).type_string(), "Long Array");
    /// ```
    pub fn type_string(&self) -> &'static str {
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
            NbtTag::List(_) => "List",
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
    /// # use nbt::NbtTag;
    /// assert_eq!(NbtTag::Byte(5).to_snbt(), "5B");
    /// assert_eq!(NbtTag::StringModUtf8("\"Quoted text\"".to_owned()).to_snbt(), "'\"Quoted text\"'");
    /// ```
    ///
    /// More complex tag conversion:
    ///
    /// ```
    /// # use nbt::*;
    /// let mut compound = NbtCompound::new();
    /// compound.set("foo".to_owned(), vec![-1_i64, -3_i64, -5_i64]);
    /// assert_eq!(NbtTag::Compound(compound).to_snbt(), "{foo:[L;-1,-3,-5]}");
    /// ```
    pub fn to_snbt(&self) -> String {
        macro_rules! list_to_string {
            ($list:expr) => {
                format!(
                    "[{};{}]",
                    self.type_specifier(),
                    $list
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<String>>()
                        .join(",")
                )
            };
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
            NbtTag::LongArray(value) => list_to_string!(value),
        }
    }

    /// Returns whether or not the given string needs to be quoted due to non-alphanumeric or otherwise
    /// non-standard characters.
    pub fn should_quote(string: &str) -> bool {
        for ch in string.chars() {
            if ch == ':'
                || ch == ','
                || ch == '"'
                || ch == '\''
                || ch == '{'
                || ch == '}'
                || ch == '['
                || ch == ']'
            {
                return true;
            }
        }

        false
    }

    /// Wraps the given string in quotes and escapes any quotes contained in the original string.
    pub fn string_to_snbt(string: &str) -> String {
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

impl Display for NbtTag {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(&self.to_snbt(), f)
    }
}

impl Debug for NbtTag {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Debug::fmt(&self.to_snbt(), f)
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
                type Error = NbtStructureError;

                fn try_from(tag: &NbtTag) -> Result<Self, Self::Error> {
                    if let NbtTag::$tag(value) = tag {
                        Ok(*value)
                    } else {
                        Err(NbtStructureError::TypeMismatch)
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
    type Error = NbtStructureError;

    fn try_from(tag: &NbtTag) -> Result<Self, Self::Error> {
        match tag {
            NbtTag::Byte(value) => Ok(*value != 0),
            NbtTag::Short(value) => Ok(*value != 0),
            NbtTag::Int(value) => Ok(*value != 0),
            NbtTag::Long(value) => Ok(*value != 0),
            _ => Err(NbtStructureError::TypeMismatch),
        }
    }
}

macro_rules! ref_from_tag {
    ($($type:ty, $tag:ident);*) => {
        $(
            impl<'a> TryFrom<&'a NbtTag> for &'a $type {
                type Error = NbtStructureError;

                fn try_from(tag: &'a NbtTag) -> Result<Self, Self::Error> {
                    if let NbtTag::$tag(value) = tag {
                        Ok(value)
                    } else {
                        Err(NbtStructureError::TypeMismatch)
                    }
                }
            }

            impl<'a> TryFrom<&'a mut NbtTag> for &'a mut $type {
                type Error = NbtStructureError;

                fn try_from(tag: &'a mut NbtTag) -> Result<Self, Self::Error> {
                    if let NbtTag::$tag(value) = tag {
                        Ok(value)
                    } else {
                        Err(NbtStructureError::TypeMismatch)
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

macro_rules! from_tag {
    ($($type:ty, $tag:ident);*) => {
        $(
            impl TryFrom<NbtTag> for $type {
                type Error = NbtStructureError;

                fn try_from(tag: NbtTag) -> Result<Self, Self::Error> {
                    if let NbtTag::$tag(value) = tag {
                        Ok(value)
                    } else {
                        Err(NbtStructureError::TypeMismatch)
                    }
                }
            }
        )*
    };
}

from_tag!(
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

/// The NBT tag list type which is essentially just a wrapper for a vec of NBT tags.
#[repr(transparent)]
#[derive(Clone)]
pub struct NbtList(Vec<NbtTag>);

impl NbtList {
    /// Returns a new NBT tag list with an empty internal vec.
    pub fn new() -> Self {
        NbtList(Vec::new())
    }

    /// Returns the internal vector of this NBT list.
    pub fn into_inner(self) -> Vec<NbtTag> {
        self.0
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
        &'a L: IntoIterator<Item = &'a T>,
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
        &'a L: IntoIterator<Item = &'a T>,
    {
        NbtList(list.into_iter().map(|x| x.to_nbt().into()).collect())
    }

    /// Iterates over this tag list, converting each tag reference into the specified type.
    ///
    /// # Examples
    ///
    /// ```
    /// # use nbt::{NbtList, NbtStructureError};
    /// let mut list = NbtList::new();
    /// list.add(0i32);
    /// list.add(1i32);
    /// list.add(2.0f64);
    ///
    /// let mut iter = list.iter_map::<_, i32>();
    /// assert_eq!(Some(Ok(0i32)), iter.next());
    /// assert_eq!(Some(Ok(1i32)), iter.next());
    /// assert_eq!(Some(Err(NbtStructureError::TypeMismatch)), iter.next());
    /// assert_eq!(None, iter.next());
    /// ```
    pub fn iter_map<'a, E, T: TryFrom<&'a NbtTag, Error = E>>(
        &'a self,
    ) -> impl Iterator<Item = Result<T, E>> + 'a {
        self.0.iter().map(|tag| T::try_from(tag))
    }

    /// Iterates over mutable references to the tags in this list, converting each tag reference into
    /// the specified type. See [`iter_map`](crate::tag::NbtList::iter_map) for usage details.
    pub fn iter_mut_map<'a, E, T: TryFrom<&'a mut NbtTag, Error = E>>(
        &'a mut self,
    ) -> impl Iterator<Item = Result<T, E>> + 'a {
        self.0.iter_mut().map(|tag| T::try_from(tag))
    }

    /// Iterates over this tag list, converting each tag into an owned value of the given concrete type.
    pub fn iter_into_repr<T: NbtRepr>(
        &self,
    ) -> impl Iterator<Item = Result<T, NbtReprError<T::Error>>> + '_ {
        self.0
            .iter()
            .map(|tag| T::from_nbt(tag.try_into()?).map_err(NbtReprError::custom))
    }

    /// Converts this tag list to a valid SNBT string.
    pub fn to_snbt(&self) -> String {
        let mut snbt_list = String::with_capacity(2 + 8 * self.len());
        snbt_list.push('[');
        snbt_list.push_str(
            &self
                .as_ref()
                .iter()
                .map(|tag| tag.to_snbt())
                .collect::<Vec<String>>()
                .join(","),
        );
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
    pub fn get<'a, T: TryFrom<&'a NbtTag>>(
        &'a self,
        index: usize,
    ) -> Result<T, NbtReprError<T::Error>>
    {
        T::try_from(self.0.get(index).ok_or(NbtStructureError::InvalidIndex)?)
            .map_err(NbtReprError::custom)
    }

    /// Returns a mutable reference to the tag at the given index, or `None` if the index is out of bounds. This
    /// method should be used for obtaining mutable references to lists and compounds.
    pub fn get_mut<'a, T: TryFrom<&'a mut NbtTag>>(
        &'a mut self,
        index: usize,
    ) -> Result<T, NbtReprError<T::Error>>
    {
        T::try_from(
            self.0
                .get_mut(index)
                .ok_or(NbtStructureError::InvalidIndex)?,
        )
        .map_err(NbtReprError::custom)
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

impl Display for NbtList {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(&self.to_snbt(), f)
    }
}

impl Debug for NbtList {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Debug::fmt(&self.to_snbt(), f)
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

    /// Returns the internal hash map of this NBT compound.
    pub fn into_inner(self) -> HashMap<String, NbtTag> {
        self.0
    }

    /// Returns a new NBT tag compound with the given initial capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        NbtCompound(HashMap::with_capacity(capacity))
    }

    /// Clones the data in the given map and converts it into an [`NbtCompound`](crate::tag::NbtCompound).
    ///
    /// # Examples
    ///
    /// ```
    /// # use nbt::NbtCompound;
    /// # use std::collections::HashMap;
    /// let mut map = HashMap::new();
    /// map.insert("foo", 10i32);
    /// map.insert("bar", -5i32);
    ///
    /// let compound = NbtCompound::clone_from(&map);
    /// assert_eq!(compound.get::<_, i32>("foo").unwrap() + compound.get::<_, i32>("bar").unwrap(), 5i32);
    /// ```
    pub fn clone_from<'a, K, V, M>(map: &'a M) -> Self
    where
        K: Clone + Into<String> + 'a,
        V: Clone + Into<NbtTag> + 'a,
        &'a M: IntoIterator<Item = (&'a K, &'a V)>,
    {
        NbtCompound(
            map.into_iter()
                .map(|(key, value)| (key.clone().into(), value.clone().into()))
                .collect(),
        )
    }

    /// Creates an [`NbtCompound`] of [`NbtCompound`]s by mapping each element in the given map to its
    /// NBT representation.
    ///
    /// [`NbtCompound`]: crate::tag::NbtCompound
    pub fn clone_repr_from<'a, K, V, M>(map: &'a M) -> Self
    where
        K: Clone + Into<String> + 'a,
        V: NbtRepr + 'a,
        &'a M: IntoIterator<Item = (&'a K, &'a V)>,
    {
        NbtCompound(
            map.into_iter()
                .map(|(key, value)| (key.clone().into(), value.to_nbt().into()))
                .collect(),
        )
    }

    /// Iterates over this tag compound, converting each tag reference into the specified type. Each key is
    /// paired with the result of the attempted conversion into the specified type. The iterator will not
    /// terminate even if some conversions fail.
    pub fn iter_map<'a, E, T: TryFrom<&'a NbtTag, Error = E>>(
        &'a self,
    ) -> impl Iterator<Item = (&'a str, Result<T, E>)> + 'a {
        self.0
            .iter()
            .map(|(key, tag)| (key.as_str(), T::try_from(tag)))
    }

    /// Iterates over this tag compound, converting each mutable tag reference into the specified type. See
    /// [`iter_map`](crate::tag::NbtCompound::iter_map) for details.
    pub fn iter_mut_map<'a, E, T: TryFrom<&'a mut NbtTag, Error = E>>(
        &'a mut self,
    ) -> impl Iterator<Item = (&'a str, Result<T, E>)> + 'a {
        self.0
            .iter_mut()
            .map(|(key, tag)| (key.as_str(), T::try_from(tag)))
    }

    /// Iterates over this tag compound, converting each tag into the specified concrete type. Each key is
    /// paired with the result of the attempted conversion into the specified type. The iterator will not
    /// terminate even if some conversions fail.
    pub fn iter_into_repr<T: NbtRepr>(
        &self,
    ) -> impl Iterator<Item = (&'_ str, Result<T, NbtReprError<T::Error>>)> + '_ {
        self.0.iter().map(|(key, tag)| {
            (key.as_str(), match tag {
                NbtTag::Compound(compound) => T::from_nbt(compound).map_err(NbtReprError::custom),
                _ => Err(NbtReprError::Structure(NbtStructureError::TypeMismatch)),
            })
        })
    }

    /// Equivalent to `NbtRepr::`[`from_nbt`]`(&compound)`.
    ///
    /// [`from_nbt`]: crate::repr::NbtRepr::from_nbt
    #[inline]
    pub fn clone_into<T: NbtRepr>(&self) -> Result<T, T::Error> {
        T::from_nbt(self)
    }

    /// Converts this tag compound into a valid SNBT string.
    pub fn to_snbt(&self) -> String {
        let mut snbt_compound = String::with_capacity(2 + 16 * self.len());
        snbt_compound.push('{');
        snbt_compound.push_str(
            &self
                .as_ref()
                .iter()
                .map(|(key, tag)| {
                    if NbtTag::should_quote(key) {
                        format!("{}:{}", NbtTag::string_to_snbt(key), tag.to_snbt())
                    } else {
                        format!("{}:{}", key, tag.to_snbt())
                    }
                })
                .collect::<Vec<String>>()
                .join(","),
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
    pub fn get<'a, K, T>(&'a self, name: &K) -> Result<T, NbtReprError<T::Error>>
    where
        String: Borrow<K>,
        K: Hash + Eq + ?Sized,
        T: TryFrom<&'a NbtTag>,
    {
        T::try_from(self.0.get(name).ok_or(NbtStructureError::MissingTag)?)
            .map_err(NbtReprError::custom)
    }

    /// Returns the value of the tag with the given name, or `None` if no tag could be found with the given name.
    /// This method should be used to obtain mutable references to lists and compounds.
    pub fn get_mut<'a, K, T>(&'a mut self, name: &K) -> Result<T, NbtReprError<T::Error>>
    where
        String: Borrow<K>,
        K: Hash + Eq + ?Sized,
        T: TryFrom<&'a NbtTag>,
    {
        T::try_from(self.0.get_mut(name).ok_or(NbtStructureError::MissingTag)?)
            .map_err(NbtReprError::custom)
    }

    /// Returns whether or not this compound has a tag with the given name.
    #[inline]
    pub fn has<K>(&self, key: &K) -> bool
    where
        String: Borrow<K>,
        K: Hash + Eq + ?Sized,
    {
        self.0.contains_key(key)
    }

    /// Adds the given value to this compound with the given name after wrapping that value in an `NbtTag`.
    pub fn set<K: Into<String>, T: Into<NbtTag>>(&mut self, name: K, value: T) {
        self.0.insert(name.into(), value.into());
    }

    /// Parses a nbt compound from snbt
    ///
    /// # Example
    ///
    /// ```
    /// # use nbt::NbtCompound;
    /// let tag = NbtCompound::from_snbt(r#"{string:Stuff, list:[I;1,2,3,4,5]}"#).unwrap();
    /// assert_eq!(tag.get::<_, &str>("string"), Ok("Stuff"));
    /// assert_eq!(tag.get::<_, &[i32]>("list"), Ok(vec![1,2,3,4,5].as_slice()));
    /// ```
    pub fn from_snbt(input: &str) -> Result<Self, ParserError> {
        snbt::parse(input)
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

impl Display for NbtCompound {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(&self.to_snbt(), f)
    }
}

impl Debug for NbtCompound {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Debug::fmt(&self.to_snbt(), f)
    }
}

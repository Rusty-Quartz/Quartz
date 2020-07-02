use std::collections::HashMap;
use std::ops::{Index, IndexMut};
use std::iter::*;
use std::fmt;

use chat::{
    Component,
    TextComponentBuilder,
    color::PredefinedColor,
    component::TextComponent
};
use doc_comment::doc_comment;

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

// Formats some kind of list to the given formatter
macro_rules! to_component {
    () => {
        #[doc = "Converts this tag entity into a formatted text component designed for user-friendly displaying of NBT data."]
        pub fn to_component(&self) -> Component {
            let mut text_component = TextComponent::new(String::new(), Some(PredefinedColor::White.into_color()));
            text_component.extra = Some(self.to_component_parts());
            Component::Text(text_component)
        }
    };
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

    /// Converts this tag into a sequence of components which when displayed together will depict
    /// this tag's data in a user-friendly form.
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

    to_component!();
}

// Display the tag in a user-friendly form
impl fmt::Display for NbtTag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_component())
    }
}

// Implement the from trait for all the tag's internal types
macro_rules! tag_from {
    ($($type:ty, $tag:ident),*) => {
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
    i8, Byte,
    i16, Short,
    i32, Int,
    i64, Long,
    f32, Float,
    f64, Double,
    Vec<i8>, ByteArray,
    String, StringModUtf8,
    NbtList, List,
    NbtCompound, Compound,
    Vec<i32>, IntArray,
    Vec<i64>, LongArray
);

// String slices are a special case
impl From<&str> for NbtTag {
    fn from(value: &str) -> NbtTag {
        NbtTag::StringModUtf8(value.to_owned())
    }
}

/// The NBT tag list type which is essentially just a wrapper for a vec of NBT tags.
#[repr(transparent)]
#[derive(Clone)]
pub struct NbtList(Vec<NbtTag>);

// Gets an element from the list, returning a default value if the types do not match
macro_rules! list_get {
    ($type:ty, $method:ident, $tag:ident) => {
        doc_comment! {
            concat!(
                "
                Returns the value of a `", stringify!($tag), "` tag at the given index.
                If the index is out of bounds, or the tag is not a `", stringify!($tag), "` tag, then `None` is returned.
                "
            ),
            pub fn $method(&self, index: usize) -> Option<$type> {
                if let Some(NbtTag::$tag(value)) = self.0.get(index) {
                    Some(*value)
                } else {
                    None
                }
            }
        }
    };
}

// Generates get and get_mut functions that return references to tag values
// Returns None on a type mismatch
macro_rules! list_get_ref {
    ($type:ty, $method:ident, $method_mut:ident, $tag:ident) => {
        doc_comment! {
            concat!(
                "
                Returns a shared reference to the value of a `", stringify!($tag), "` tag at the given index.
                If the index is out of bounds, or the tag is not a `", stringify!($tag), "` tag, then `None` is returned.
                "
            ),
            pub fn $method(&self, index: usize) -> Option<&$type> {
                if let Some(NbtTag::$tag(value)) = self.0.get(index) {
                    Some(value)
                } else {
                    None
                }
            }
        }

        doc_comment! {
            concat!(
                "
                Returns a mutable reference to the value of a `", stringify!($tag), "` tag at the given index.
                If the index is out of bounds, or the tag is not a `", stringify!($tag), "` tag, then `None` is returned.
                "
            ),
            pub fn $method_mut(&mut self, index: usize) -> Option<&mut $type> {
                if let Some(NbtTag::$tag(value)) = self.0.get_mut(index) {
                    Some(value)
                } else {
                    None
                }
            }
        }
    };
}

// Generates a function for wrapping a rust type in a nbt tag and adding it to the list
macro_rules! list_add {
    ($type:ty, $method:ident) => {
        #[doc = "Adds the given value to the back of the list. The value will be wrapped in a corresponding `NbtTag` variant."]
        pub fn $method(&mut self, value: $type) {
            self.0.push(NbtTag::from(value));
        }
    };
}

impl NbtList {
    /// Returns a new NBT tag list with an empty internal vec.
    pub fn new() -> Self {
        NbtList(Vec::new())
    }

    /// Returns a new NBT tag list with the given initial capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        NbtList(Vec::with_capacity(capacity))
    }

    /// Converts this tag list to a valid SNBT string.
    pub fn to_snbt(&self) -> String {
        let mut snbt_list = String::with_capacity(2 + 8 * self.len());
        snbt_list.push('[');
        snbt_list.push_str(&self.iter().map(|tag| tag.to_snbt()).collect::<Vec<String>>().join(","));
        snbt_list.push(']');
        snbt_list
    }

    /// Converts this tag list into a sequence of components which when displayed together will depict
    /// this list's data in a user-friendly form.
    fn to_component_parts(&self) -> Vec<Component> {
        if self.is_empty() {
            return vec![Component::text("[]".to_owned())];
        }

        let mut components = Vec::with_capacity(2 + 3 * self.len());
        components.push(Component::text("[".to_owned()));
        components.extend(self[0].to_component_parts());

        for tag in self.iter().skip(1) {
            components.push(Component::text(", ".to_owned()));
            components.extend(tag.to_component_parts());
        }

        components.push(Component::text("]".to_owned()));
        components
    }

    to_component!();

    // The following are just calling the corresponding functions in the underlying vec

    /// Returns an interator over shared references to tags in this list.
    #[inline(always)]
    pub fn iter(&self) -> std::slice::Iter<'_, NbtTag> {
        self.0.iter()
    }

    /// Returns an interator over mutable references to tags in this list.
    #[inline(always)]
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, NbtTag> {
        self.0.iter_mut()
    }

    /// Returns the length of this list.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if this tag list has a length of zero, false otherwise.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Removes and returns the tag at the given index. Unlike a Vec, this method will not panic
    /// if the index is out of bounds, rather `None` will be returned.
    #[inline(always)]
    pub fn remove(&mut self, index: usize) -> Option<NbtTag> {
        if index < self.0.len() {
            Some(self.0.remove(index))
        } else {
            None
        }
    }

    /// Returns a reference to the tag at the given index, or None if the index is out of bounds.
    pub fn get(&self, index: usize) -> Option<&NbtTag> {
        self.0.get(index)
    }

    // Generate the get functions
    list_get!(i8, get_byte, Byte);
    list_get!(i16, get_short, Short);
    list_get!(i32, get_int, Int);
    list_get!(i64, get_long, Long);
    list_get!(f32, get_float, Float);
    list_get!(f64, get_double, Double);
    list_get_ref!(Vec<i8>, get_byte_array, get_byte_array_mut, ByteArray);
    list_get_ref!(str, get_string, get_string_mut, StringModUtf8);
    list_get_ref!(NbtList, get_list, get_list_mut, List);
    list_get_ref!(NbtCompound, get_compound, get_compound_mut, Compound);
    list_get_ref!(Vec<i32>, get_int_array, get_int_array_mut, IntArray);
    list_get_ref!(Vec<i64>, get_long_array, get_long_array_mut, LongArray);

    /// Returns whether or not an integer-type tag at the given index has a value other than zero. If the
    /// index is out of bounds or the tag at the given index is not an integer type, then `None` is returned.
    pub fn get_bool(&self, index: usize) -> Option<bool> {
        match self.0.get(index) {
            Some(NbtTag::Byte(value)) => Some(*value != 0),
            Some(NbtTag::Short(value)) => Some(*value != 0),
            Some(NbtTag::Int(value)) => Some(*value != 0),
            Some(NbtTag::Long(value)) => Some(*value != 0),
            _ => None
        }
    }

    /// Pushes the given `NbtTag` to the back of the list.
    pub fn add(&mut self, tag: NbtTag) {
        self.0.push(tag);
    }

    // Generate the add functions
    list_add!(i8, add_byte);
    list_add!(i16, add_short);
    list_add!(i32, add_int);
    list_add!(i64, add_long);
    list_add!(f32, add_float);
    list_add!(f64, add_double);
    list_add!(Vec<i8>, add_byte_array);
    list_add!(String, add_string);
    list_add!(NbtList, add_list);
    list_add!(NbtCompound, add_compound);
    list_add!(Vec<i32>, add_int_array);
    list_add!(Vec<i64>, add_long_array);

    /// Adds a byte tag with value `1` if the given boolean is true, otherwise a byte tag with value `0` is added.
    pub fn add_bool(&mut self, value: bool) {
        if value {
            self.add_byte(1);
        } else {
            self.add_byte(0);
        }
    }
}

impl fmt::Display for NbtList {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_component())
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

// Generates a get function for a compound returning a default value if
// the name is invalid or the types do not match
macro_rules! compound_get {
    ($type:ty, $method:ident, $tag:ident) => {
        doc_comment! {
            concat!(
                "
                Returns the value of the `", stringify!($tag), "` tag with the given name.
                If a tag with the given name cannot be found, or the tag is not a `", stringify!($tag), "` tag,
                then `None` is returned.
                "
            ),
            pub fn $method(&self, name: &str) -> Option<$type> {
                if let Some(NbtTag::$tag(value)) = self.0.get(name) {
                    Some(*value)
                } else {
                    None
                }
            }
        }
    };
}

// Generates get and get_mut functions returning references to tag value
// or None if the tag name is invalid or the types do not match
macro_rules! compound_get_ref {
    ($type:ty, $method:ident, $method_mut:ident, $tag:ident) => {
        doc_comment! {
            concat!(
                "
                Returns a shared reference to the value of the `", stringify!($tag), "` tag with the given name.
                If a tag with the given name cannot be found, or the tag is not a `", stringify!($tag), "` tag,
                then `None` is returned.
                "
            ),
            pub fn $method(&self, name: &str) -> Option<&$type> {
                if let Some(NbtTag::$tag(value)) = self.0.get(name) {
                    Some(value)
                } else {
                    None
                }
            }
        }

        doc_comment! {
            concat!(
                "
                Returns a mutable reference to the value of the `", stringify!($tag), "` tag with the given name.
                If a tag with the given name cannot be found, or the tag is not a `", stringify!($tag), "` tag,
                then `None` is returned.
                "
            ),
            pub fn $method_mut(&mut self, name: &str) -> Option<&mut $type> {
                if let Some(NbtTag::$tag(value)) = self.0.get_mut(name) {
                    Some(value)
                } else {
                    None
                }
            }
        }
    };
}

// Generates an insert function that wraps the given rust type in an nbt tag
// and inserts it into the inner map
macro_rules! compound_insert {
    ($type:ty, $method:ident) => {
        #[doc =
            "
            Inserts a tag with the given value into this compound with the given name. 
            The value will be wrapped in a corresponding `NbtTag` variant.
            "
        ]
        pub fn $method(&mut self, name: String, value: $type) {
            self.0.insert(name, NbtTag::from(value));
        }
    };
}

impl NbtCompound {
    /// Returns a new NBT tag compound with an empty internal hash map.
    pub fn new() -> Self {
        NbtCompound(HashMap::new())
    }

    /// Returns a new NBT tag compound with the given initial capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        NbtCompound(HashMap::with_capacity(capacity))
    }

    /// Converts this tag compound into a valid SNBT string.
    pub fn to_snbt(&self) -> String {
        let mut snbt_compound = String::with_capacity(2 + 16 * self.len());
        snbt_compound.push('{');
        snbt_compound.push_str(
            &self.iter()
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

    /// Converts this tag compound into a sequence of components which when displayed together will depict
    /// this compound's data in a user-friendly form.
    fn to_component_parts(&self) -> Vec<Component> {
        if self.is_empty() {
            return vec![Component::text("{}".to_owned())];
        }

        let mut components = Vec::with_capacity(2 + 3 * self.len());

        // Grab the elements and push the first one
        let elements = self.iter().collect::<Vec<(&String, &NbtTag)>>();
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

    to_component!();

    // The following just call the corresponding hash map functions

    /// Returns an iterator over the keys of this tag compound.
    #[inline(always)]
    pub fn keys(&self) -> std::collections::hash_map::Keys<'_, String, NbtTag> {
        self.0.keys()
    }

    /// Returns an iterator over the values of this tag compound.
    #[inline(always)]
    pub fn values(&self) -> std::collections::hash_map::Values<'_, String, NbtTag> {
        self.0.values()
    }

    /// Returns an iterator over mutable references to the values of this tag compound.
    #[inline(always)]
    pub fn values_mut(&mut self) -> std::collections::hash_map::ValuesMut<'_, String, NbtTag> {
        self.0.values_mut()
    }

    /// Returns an iterator over the key-value pairs of this tag compound.
    #[inline(always)]
    pub fn iter(&self) -> std::collections::hash_map::Iter<'_, String, NbtTag> {
        self.0.iter()
    }

    /// Returns an iterator over mutable references to the key-value pairs of this tag compound.
    #[inline(always)]
    pub fn iter_mut(&mut self) -> std::collections::hash_map::IterMut<'_, String, NbtTag> {
        self.0.iter_mut()
    }

    /// Returns the number of tags in this compound.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if the length of this compound is zero, false otherwise.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Removes the tag with the given name, returning that tag. If a tag with the given name cannot be
    /// found, then `None` is returned.
    #[inline(always)]
    pub fn remove(&mut self, name: &str) -> Option<NbtTag> {
        self.0.remove(name)
    }

    /// Returns a shared reference to the tag with the given name, or `None` if there is no such tag
    /// with the given name.
    pub fn get(&self, name: &str) -> Option<&NbtTag> {
        self.0.get(name)
    }

    // Generate the get functions
    compound_get!(i8, get_byte, Byte);
    compound_get!(i16, get_short, Short);
    compound_get!(i32, get_int, Int);
    compound_get!(i64, get_long, Long);
    compound_get!(f32, get_float, Float);
    compound_get!(f64, get_double, Double);
    compound_get_ref!(Vec<i8>, get_byte_array, get_byte_array_mut, ByteArray);
    compound_get_ref!(str, get_string, get_string_mut, StringModUtf8);
    compound_get_ref!(NbtList, get_list, get_list_mut, List);
    compound_get_ref!(NbtCompound, get_compound, get_compound_mut, Compound);
    compound_get_ref!(Vec<i32>, get_int_array, get_int_array_mut, IntArray);
    compound_get_ref!(Vec<i64>, get_long_array, get_long_array_mut, LongArray);

    /// Returns whether or not an integer-type tag with the given name has a value other than zero. If there is no
    /// tag with the given name, or the tag is not an integer type, then `None` is returned.
    pub fn get_bool(&self, name: &str) -> Option<bool> {
        match self.0.get(name) {
            Some(tag) => {
                match *tag {
                    NbtTag::Byte(value) => Some(value != 0),
                    NbtTag::Short(value) => Some(value != 0),
                    NbtTag::Int(value) => Some(value != 0),
                    NbtTag::Long(value) => Some(value != 0),
                    _ => None
                }
            },
            None => None
        }
    }

    /// Returns whether or not this compound has a tag with the given name.
    pub fn has(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    /// Adds the given `NbtTag` to this compound with the given name.
    pub fn set(&mut self, name: String, tag: NbtTag) {
        self.0.insert(name, tag);
    }

    // Generate set functions
    compound_insert!(i8, set_byte);
    compound_insert!(i16, set_short);
    compound_insert!(i32, set_int);
    compound_insert!(i64, set_long);
    compound_insert!(f32, set_float);
    compound_insert!(f64, set_double);
    compound_insert!(Vec<i8>, set_byte_array);
    compound_insert!(String, set_string);
    compound_insert!(NbtList, set_list);
    compound_insert!(NbtCompound, set_compound);
    compound_insert!(Vec<i32>, set_int_array);
    compound_insert!(Vec<i64>, set_long_array);

    /// Inserts a byte tag with the given name, and with a value of `0` or `1` if the given boolean
    /// is `false` or `true` respectively.
    pub fn set_bool(&mut self, name: String, value: bool) {
        if value {
            self.set_byte(name, 1);
        } else {
            self.set_byte(name, 0);
        }
    }
}

// Display the compound as valid SNBT format
impl fmt::Display for NbtCompound {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_component())
    }
}
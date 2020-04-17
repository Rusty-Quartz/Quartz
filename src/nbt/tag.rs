use std::collections::HashMap;
use std::ops::{Index, IndexMut};
use std::iter::*;
use std::fmt;
use std::fmt::Write;

pub enum NbtTag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    StringModUtf8(String),
    List(NbtList),
    Compound(NbtCompound),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>)
}

macro_rules! write_list {
    ($formatter:expr, $list:expr, $element:expr) => {
        if $list.is_empty() {
            $formatter.write_str("[]")
        } else {
            write!($formatter, "[{}", $list[0])?;
            for ele in $list.iter().skip(1) {
                write!($formatter, $element, ele)?;
            }
            $formatter.write_char(']')
        }
    };
}

impl fmt::Display for NbtTag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            NbtTag::Byte(value) => write!(f, "{}b", value),
            NbtTag::Short(value) => write!(f, "{}s", value),
            NbtTag::Int(value) => write!(f, "{}", value),
            NbtTag::Long(value) => write!(f, "{}L", value),
            NbtTag::Float(value) => write!(f, "{}f", value),
            NbtTag::Double(value) => write!(f, "{}d", value),
            NbtTag::ByteArray(value) => write_list!(f, value, ", {}b"),
            NbtTag::StringModUtf8(value) => {
                let surrounding: char;
                if value.contains("\"") {
                    surrounding = '\'';
                } else {
                    surrounding = '\"';
                }

                f.write_char(surrounding)?;
                for ch in value.chars() {
                    if ch == surrounding || ch == '\\' {
                        f.write_char('\\')?;
                    }
                    f.write_char(ch)?;
                }
                f.write_char(surrounding)
            },
            NbtTag::List(value) => write!(f, "{}", value),
            NbtTag::Compound(value) => write!(f, "{}", value),
            NbtTag::IntArray(value) => write_list!(f, value, ", {}"),
            NbtTag::LongArray(value) => write_list!(f, value, ", {}L")
        }
    }
}

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

impl<'a> From<&'a str> for NbtTag {
    fn from(value: &'a str) -> NbtTag {
        NbtTag::StringModUtf8(value.into())
    }
}

#[repr(transparent)]
pub struct NbtList(Vec<NbtTag>);

macro_rules! list_get {
    ($type:ty, $method:ident, $tag:ident, $default:expr) => {
        pub fn $method(&self, index: usize) -> $type {
            if let NbtTag::$tag(value) = self.0[index] {
                value
            } else {
                $default
            }
        }
    };
    ($type:ty, $method:ident, $tag:ident) => {
        list_get!($type, $method, $tag, 0);
    };
}

macro_rules! list_get_ref {
    ($type:ty, $method:ident, $method_mut:ident, $tag:ident) => {
        pub fn $method(&self, index: usize) -> Option<&$type> {
            if let NbtTag::$tag(value) = &self.0[index] {
                Some(value)
            } else {
                None
            }
        }

        pub fn $method_mut(&mut self, index: usize) -> Option<&mut $type> {
            if let NbtTag::$tag(value) = &mut self.0[index] {
                Some(value)
            } else {
                None
            }
        }
    };
}

macro_rules! list_add {
    ($type:ty, $method:ident) => {
        pub fn $method(&mut self, value: $type) {
            self.0.push(NbtTag::from(value));
        }
    };
}

impl NbtList {
    pub fn new() -> Self {
        NbtList(Vec::new())
    }

    pub fn with_capacity(capacity: usize) -> Self {
        NbtList(Vec::with_capacity(capacity))
    }

    #[inline(always)]
    pub fn iter(&self) -> std::slice::Iter<'_, NbtTag> {
        self.0.iter()
    }

    #[inline(always)]
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, NbtTag> {
        self.0.iter_mut()
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[inline(always)]
    pub fn remove(&mut self, index: usize) -> NbtTag {
        self.0.remove(index)
    }

    list_get!(i8, get_byte, Byte);
    list_get!(i16, get_short, Short);
    list_get!(i32, get_int, Int);
    list_get!(i64, get_long, Long);
    list_get!(f32, get_float, Float, 0.0);
    list_get!(f64, get_double, Double, 0.0);
    list_get_ref!(Vec<i8>, get_byte_array, get_byte_array_mut, ByteArray);
    list_get_ref!(NbtList, get_list, get_list_mut, List);
    list_get_ref!(NbtCompound, get_compound, get_compound_mut, Compound);
    list_get_ref!(Vec<i32>, get_int_array, get_int_array_mut, IntArray);
    list_get_ref!(Vec<i64>, get_long_array, get_long_array_mut, LongArray);

    pub fn get_string(&self, index: usize) -> &str {
        if let NbtTag::StringModUtf8(value) = &self.0[index] {
            value
        } else {
            ""
        }
    }

    pub fn get_bool(&self, index: usize) -> bool {
        self.get_byte(index) != 0
    }

    pub fn add(&mut self, tag: NbtTag) {
        self.0.push(tag);
    }

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
        write_list!(f, self, ", {}")
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

#[repr(transparent)]
pub struct NbtCompound(HashMap<String, NbtTag>);

macro_rules! compound_get {
    ($type:ty, $method:ident, $tag:ident, $default:expr) => {
        pub fn $method(&self, name: &str) -> $type {
            if let Some(NbtTag::$tag(value)) = self.0.get(name) {
                *value
            } else {
                $default
            }
        }
    };
    ($type:ty, $method:ident, $tag:ident) => {
        compound_get!($type, $method, $tag, 0);
    };
}

macro_rules! compound_get_ref {
    ($type:ty, $method:ident, $method_mut:ident, $tag:ident) => {
        pub fn $method(&self, name: &str) -> Option<&$type> {
            if let Some(NbtTag::$tag(value)) = self.0.get(name) {
                Some(value)
            } else {
                None
            }
        }

        pub fn $method_mut(&mut self, name: &str) -> Option<&mut $type> {
            if let Some(NbtTag::$tag(value)) = self.0.get_mut(name) {
                Some(value)
            } else {
                None
            }
        }
    };
}

macro_rules! compound_insert {
    ($type:ty, $method:ident) => {
        pub fn $method(&mut self, name: String, value: $type) {
            self.0.insert(name, NbtTag::from(value));
        }
    };
}

impl NbtCompound {
    pub fn new() -> Self {
        NbtCompound(HashMap::new())
    }

    #[inline(always)]
    pub fn keys(&self) -> std::collections::hash_map::Keys<'_, String, NbtTag> {
        self.0.keys()
    }

    #[inline(always)]
    pub fn values(&self) -> std::collections::hash_map::Values<'_, String, NbtTag> {
        self.0.values()
    }

    #[inline(always)]
    pub fn iter(&self) -> std::collections::hash_map::Iter<'_, String, NbtTag> {
        self.0.iter()
    }

    #[inline(always)]
    pub fn iter_mut(&mut self) -> std::collections::hash_map::IterMut<'_, String, NbtTag> {
        self.0.iter_mut()
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[inline(always)]
    pub fn remove(&mut self, name: &str) -> Option<NbtTag> {
        self.0.remove(name)
    }

    compound_get!(i8, get_byte, Byte);
    compound_get!(i16, get_short, Short);
    compound_get!(i32, get_int, Int);
    compound_get!(i64, get_long, Long);
    compound_get!(f32, get_float, Float, 0.0);
    compound_get!(f64, get_double, Double, 0.0);
    compound_get_ref!(Vec<i8>, get_byte_array, get_byte_array_mut, ByteArray);
    compound_get_ref!(NbtList, get_list, get_list_mut, List);
    compound_get_ref!(NbtCompound, get_compound, get_compound_mut, Compound);
    compound_get_ref!(Vec<i32>, get_int_array, get_int_array_mut, IntArray);
    compound_get_ref!(Vec<i64>, get_long_array, get_long_array_mut, LongArray);

    pub fn get_string(&self, name: &str) -> &str {
        if let Some(NbtTag::StringModUtf8(value)) = self.0.get(name) {
            value
        } else {
            ""
        }
    }

    pub fn get_bool(&self, name: &str) -> bool {
        return self.get_byte(name) != 0;
    }

    pub fn set(&mut self, name: String, tag: NbtTag) {
        self.0.insert(name, tag);
    }

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

    pub fn set_bool(&mut self, name: String, value: bool) {
        if value {
            self.set_byte(name, 1);
        } else {
            self.set_byte(name, 0);
        }
    }
}

impl fmt::Display for NbtCompound {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_empty() {
            f.write_str("{}")
        } else {
            f.write_char('{')?;
            let mut i: usize = 0;
            for (name, tag) in self.iter() {
                if i < self.len() - 1 {
                    write!(f, "{}: {}, ", name, tag)?;
                } else {
                    write!(f, "{}: {}", name, tag)?;
                }
                i += 1;
            }
            f.write_char('}')
        }
    }
}
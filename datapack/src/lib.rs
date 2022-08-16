//! quartz_datapack is a crate to read and write minecraft datapacks
//!
//! Note: To avoid the amount of single-use datatypes some types have notes ensuring the usage is valid<br>
//! If these notes are not followed then the datapack will not be valid by vanilla's standards<br>
//! To help with this, manual Serialize and Deserialize implementations will enforce these rules<br>
//! Though, where custom implementations were not already necessary for the datatype these were not added

/// Unwraps the fields and returns a missing_field error if they are `None`
macro_rules! missing_field_error {
    ($($field: ident, $field_str: literal),*) => {
        $(let $field = match $field {
            Some(f) => f,
            None => return Err(serde::de::Error::missing_field($field_str)),
        };)*
    };
}

/// Generates a map visitor to deserialize a map with the given fields
macro_rules! map_visitor {
    ($map: ident, $(($field: ident, $field_str: literal, $field_type: ty)),*) => {
        $(let mut $field = None;)*

        while let Some(__key__) = $map.next_key()? {
            match __key__ {
                $($field_str => {
                    if $field.is_some() {
                        return Err(serde::de::Error::duplicate_field($field_str))
                    }

                    $field = Some($map.next_value::<$field_type>()?);
                },)*
                _ => return Err(serde::de::Error::unknown_field(__key__, &[$($field_str),*]))
            }
        }
    };
}

mod datapack;
pub use datapack::*;
pub mod data;

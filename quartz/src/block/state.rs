use std::fmt::{self, Display, Formatter};
use lazy_static::lazy_static;
use tinyvec::ArrayVec;
use util::UnlocalizedName;

/// A type alias for the numeric block state type, currently `u16`.
pub type StateID = u16;

/// A specific block type, not to be confused with a block state which specifies variants of a type. This
/// is used as a data handle for block states.
pub struct Block {
    pub name: UnlocalizedName,
    pub properties: ArrayVec<[(String, Vec<String>); 16]>,
    pub base_state: StateID,
    pub default_state: StateID
}

impl Display for Block {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.name.fmt(f)
    }
}

pub trait BlockState {
    fn handle(&self) -> &Block;

    fn id(&self) -> StateID;
}

// TODO: Implement static block state
pub struct StaticBlockState {
}

impl BlockState for StaticBlockState {
    fn handle(&self) -> &Block {
        unimplemented!();
    }

    fn id(&self) -> StateID {
        0
    }
}

#[derive(Clone)]
pub struct DynamicBlockState {
    pub handle: &'static Block,
    pub properties: ArrayVec<[(String, String); 16]>
}

impl DynamicBlockState {
    // Computes the ID for this block state, guaranteed not to fail even if the state is corrupted
    pub fn id(&self) -> StateID {
        lazy_static! {
            static ref DEFAULT_PROPERTY_VALUE: String = "".to_owned();
        }

        // This function works off a couple assumptions: state properties are sorted alphabetically and
        // that sorted list is used to construct the ID starting with the last property. Under these assumptions
        // we can do some simple arithmetic to construct the ID using indexing.

        let mut state_id: StateID = self.handle.base_state;

        match self.properties.len() {
            0 => state_id,

            // Yes, this makes a difference
            1 => {
                let state_property_value = &self.properties[0].1;

                match self.handle.properties.iter().next() {
                    // The entry is in the form (property_name, all_property_values)
                    Some(entry) => state_id + entry.1.iter().position(|value| value == state_property_value).unwrap_or(0) as StateID,
                    None => state_id
                }
            },

            _ => {
                let mut multiplier: StateID = 1;
        
                for (state_property_index, num_property_values) in self.handle.properties.iter()
                    // Map this state's property values to their index in the block's reference properties and pass along
                    // the total number of possible property values as well
                    .map(|(property_name, all_property_values)| {
                        let state_property_value = self.properties
                            .iter()
                            .find(|(key, _)| key == property_name)
                            .map(|(_, value)| value)
                            .unwrap_or(&DEFAULT_PROPERTY_VALUE);

                        (
                            // Value index
                            all_property_values.iter().position(|value| value == state_property_value).unwrap_or(0) as StateID,
                            // Total possible values
                            all_property_values.len() as StateID
                        )
                    })
                    // This lets us treat the property indices kind of like digits of a little endian integer
                    .rev()
                {
                    // Add the property index to the state offset based on the previously added properties
                    state_id += state_property_index * multiplier;
                    // Update the offset
                    multiplier *= num_property_values;
                }
        
                state_id
            }
        }
    }
}

impl Display for DynamicBlockState {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.handle.name)?;

        if !self.properties.is_empty() {
            write!(f, "[{}]", self.properties.iter()
                .map(|(property, value)| format!("{}={}", property, value))
                .collect::<Vec<String>>().join(","))?;
        }

        Ok(())
    }
}

pub struct StateBuilder {
    state: DynamicBlockState
}

impl StateBuilder {
    pub fn new(base: &DynamicBlockState) -> Self {
        StateBuilder {
            state: base.clone()
        }
    }

    pub fn add_property(&mut self, name: &str, value: &str) -> Result<(), String> {
        match self.state.properties.iter_mut().enumerate().find(|(_, (key, _))| name == key) {
            Some((index, (value_mut, _))) => {
                match self.state.handle.properties.get(index) {
                    Some((_, accepted_values)) => {
                        let owned_value = value.to_owned();

                        // Make sure the value being added is valid
                        if accepted_values.contains(&owned_value) {
                            *value_mut = owned_value;
                            Ok(())
                        } else {
                            Err(format!("Invalid property value for {} in {}: {}", name, self.state.handle.name, value))
                        }
                    },

                    None => Err(format!("Encountered corrupted state while building {}", self.state.handle.name))
                }
            },

            None => Err(format!("Invalid property for {}: {}", self.state.handle.name, name))
        }
    }

    pub fn with_property(mut self, name: &str, value: &str) -> Result<Self, (Self, String)> {
        match self.add_property(name, value) {
            Ok(()) => Ok(self),
            Err(message) => Err((self, message))
        }
    }

    pub fn with_property_unchecked(mut self, name: &str, value: &str) -> Self {
        self.state.properties.iter_mut().find(|(key, v)| name == key).unwrap().1 = value.to_owned();
        self
    }

    pub fn build(self) -> DynamicBlockState {
        self.state
    }
}
use std::collections::BTreeMap;
use std::fmt;
use lazy_static::lazy_static;
use util::UnlocalizedName;

pub type StateID = u16;

pub struct Block {
    pub name: UnlocalizedName,
    pub properties: BTreeMap<String, Vec<String>>,
    pub base_state: StateID,
    pub default_state: StateID
}

impl fmt::Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Clone)]
pub struct BlockState {
    pub handle: &'static Block,
    pub properties: BTreeMap<String, String>
}

impl BlockState {
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
                let state_property_value = self.properties.iter().next().unwrap().1;

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
                        let state_property_value = self.properties.get(property_name).unwrap_or(&DEFAULT_PROPERTY_VALUE);

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

impl fmt::Display for BlockState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
    state: BlockState
}

impl StateBuilder {
    pub fn new(base: &BlockState) -> Self {
        StateBuilder {
            state: base.clone()
        }
    }

    pub fn with_property(mut self, name: &str, value: &str) -> Result<Self, String> {
        match self.state.properties.get_mut(name) {
            // The property has to already exist in the state
            Some(val) => match self.state.handle.properties.get(name) {
                Some(accepted_values) => {
                    let owned_value = value.to_owned();

                    // Make sure the value being added is valid
                    if !accepted_values.contains(&owned_value) {
                        Err(format!("Invalid property value for {} in {}: {}", name, self.state.handle.name, value))
                    } else {
                        *val = owned_value;
                        Ok(self)
                    }
                },

                // This should never happen unless something is really off
                None => Err(format!("Encountered corrupted state while building {}", self.state.handle.name))
            },

            // If the property does not exist already then it does not exist for this type of block
            None => Err(format!("Invalid property for {}: {}", self.state.handle.name, name))
        }
    }

    pub fn with_property_unchecked(mut self, name: &str, value: &str) -> Self {
        *self.state.properties.get_mut(name).unwrap() = value.to_owned();
        self
    }

    pub fn build(self) -> BlockState {
        self.state
    }
}
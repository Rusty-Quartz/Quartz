use std::collections::{BTreeMap, HashMap};
use std::fmt;
use crate::data::UnlocalizedName;

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
    pub properties: HashMap<String, String>
}

impl BlockState {
    pub fn id(&self) -> StateID {
        // This function works off a couple assumptions: state properties are sorted alphabetically and
        // that sorted list is used to construct the ID starting with the last property. Under these assumptions
        // we can do some simple arithmetic to construct the ID using indexing.

        let mut state: u16 = self.handle.base_state;

        if !self.properties.is_empty() {
            let mut multiplier: u16 = 1;

            for (property_index, count) in self.handle.properties.iter()
                // Map this state's property values to their index in the block's reference properties and pass along
                // the total number of possible property values as well
                .map(|(property, values)| (
                    // Value index
                    values.iter().position(|value| value == self.properties.get(property).unwrap_or(&"".to_owned())).unwrap_or(0) as u16,
                    // Total possible values
                    values.len() as u16)
                )
                // This lets us treat the property indices kind of like a little endian integer
                .rev()
            {
                // Add the property index to the state offset based on the previously added properties
                state += property_index * multiplier;
                // Update the offset
                multiplier *= count;
            }
        }

        state
    }
}

impl fmt::Display for BlockState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.handle.name)?;

        if !self.properties.is_empty() {
            write!(f, "[")?;
            write!(f, "{}", self.properties.iter()
                .map(|(property, value)| format!("{}={}", property, value))
                .collect::<Vec<String>>().join(","))?;
            write!(f, "]")?;
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
            Some(val) => match self.state.handle.properties.get(name) {
                Some(accepted_values) => {
                    let owned_value = value.to_owned();
                    if !accepted_values.contains(&owned_value) {
                        Err(format!("Invalid property value for {} in {}: {}", name, self.state.handle.name, value))
                    } else {
                        *val = owned_value;
                        Ok(self)
                    }
                },
                None => Err(format!("Encountered corrupted state while building {}", self.state.handle.name))
            },
            None => Err(format!("Invalid property for {}: {}", self.state.handle.name, name))
        }
    }

    pub fn with_property_unchecked(self, name: &str, value: &str) -> Self {
        self.with_property(name, value).unwrap()
    }

    pub fn build(self) -> BlockState {
        self.state
    }
}
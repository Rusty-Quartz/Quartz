use crate::{base::registry::*, block::states::BlockStateData};
use std::fmt::{self, Debug, Display, Formatter};
use tinyvec::ArrayVec;
use util::UnlocalizedName;

/// A specific block type, not to be confused with a block state which specifies variants of a type. This
/// is used as a data handle for block states.
pub struct Block<T> {
    pub name: UnlocalizedName,
    pub properties: ArrayVec<[(String, Vec<String>); 16]>,
    pub base_state: T,
    pub default_state: T,
}

impl<T> Display for Block<T> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.name, f)
    }
}

impl<T> Debug for Block<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}

pub trait BlockState<T>: Sized {
    type Builder: StateBuilder<Self>;

    fn handle(&self) -> &Block<T>;

    fn id(&self) -> T;

    fn builder(block_name: &UnlocalizedName) -> Option<Self::Builder>;
}

// TODO: Implement static block state
#[derive(Clone, Debug)]
pub struct StaticBlockState {
    pub handle: &'static Block<StaticStateID>,
    pub data: BlockStateData,
}

impl BlockState<StaticStateID> for StaticBlockState {
    type Builder = StaticStateBuilder;

    fn handle(&self) -> &Block<StaticStateID> {
        self.handle
    }

    fn id(&self) -> StaticStateID {
        self.data.id()
    }

    fn builder(block_name: &UnlocalizedName) -> Option<Self::Builder> {
        StaticRegistry::default_state(block_name)
            .map(StaticStateBuilder::new)
    }
}

#[derive(Clone)]
pub struct DynamicBlockState {
    pub handle: &'static Block<DynamicStateID>,
    pub properties: ArrayVec<[(String, String); 16]>,
}

impl DynamicBlockState {
    // Computes the ID for this block state, guaranteed not to fail even if the state is corrupted
    pub fn id(&self) -> DynamicStateID {
        // This function works off a couple assumptions: state properties are sorted alphabetically and
        // that sorted list is used to construct the ID starting with the last property. Under these assumptions
        // we can do some simple arithmetic to construct the ID using indexing.

        let mut state_id: DynamicStateID = self.handle.base_state;

        match self.properties.len() {
            0 => state_id,

            // Yes, this makes a difference
            1 => {
                let state_property_value = &self.properties[0].1;

                match self.handle.properties.iter().next() {
                    // The entry is in the form (property_name, all_property_values)
                    Some(entry) =>
                        state_id
                            + entry
                                .1
                                .iter()
                                .position(|value| value == state_property_value)
                                .unwrap_or(0) as DynamicStateID,
                    None => state_id,
                }
            }

            _ => {
                let mut multiplier: DynamicStateID = 1;

                for (state_property_index, num_property_values) in self
                    .handle
                    .properties
                    .iter()
                    // Map this state's property values to their index in the block's reference properties and pass along
                    // the total number of possible property values as well
                    .map(|(property_name, all_property_values)| {
                        let state_property_value = self
                            .properties
                            .iter()
                            .find(|(key, _)| key == property_name)
                            .map(|(_, value)| value.as_str())
                            .unwrap_or("");

                        (
                            // Value index
                            all_property_values
                                .iter()
                                .position(|value| value == state_property_value)
                                .unwrap_or(0) as DynamicStateID,
                            // Total possible values
                            all_property_values.len() as DynamicStateID,
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
            write!(
                f,
                "[{}]",
                self.properties
                    .iter()
                    .map(|(property, value)| format!("{}={}", property, value))
                    .collect::<Vec<String>>()
                    .join(",")
            )?;
        }

        Ok(())
    }
}

pub trait StateBuilder<S>: Sized {
    fn add_property(&mut self, name: &str, value: &str) -> Result<(), String>;

    fn with_property(self, name: &str, value: &str) -> Result<Self, (Self, String)>;

    fn with_property_unchecked(self, name: &str, value: &str) -> Self {
        self.with_property(name, value)
            .map_err(|(_, message)| message)
            .unwrap()
    }

    fn build(self) -> S;
}

pub struct StaticStateBuilder {
    state: StaticBlockState,
}

impl StaticStateBuilder {
    pub fn new(base: StaticBlockState) -> Self {
        StaticStateBuilder { state: base }
    }

    fn property_error(&self, name: &str, value: &str) -> String {
        format!(
            "Invalid name or property value for {}: {}={}",
            self.state.handle.name, name, value
        )
    }
}

impl StateBuilder<StaticBlockState> for StaticStateBuilder {
    fn add_property(&mut self, name: &str, value: &str) -> Result<(), String> {
        self.state.data = self
            .state
            .data
            .with_property(name, value)
            .ok_or(self.property_error(name, value))?;
        Ok(())
    }

    fn with_property(mut self, name: &str, value: &str) -> Result<Self, (Self, String)> {
        match self.state.data.with_property(name, value) {
            Some(data) => {
                self.state.data = data;
                Ok(self)
            }
            None => {
                let msg = self.property_error(name, value);
                Err((self, msg))
            }
        }
    }

    fn with_property_unchecked(mut self, name: &str, value: &str) -> Self {
        match self.state.data.with_property(name, value) {
            Some(data) => self.state.data = data,
            None => unsafe { std::hint::unreachable_unchecked() },
        }

        self
    }

    fn build(self) -> StaticBlockState {
        self.state
    }
}

pub struct DynamicStateBuilder {
    state: DynamicBlockState,
}

impl DynamicStateBuilder {
    pub fn new(base: &DynamicBlockState) -> Self {
        DynamicStateBuilder {
            state: base.clone(),
        }
    }

    pub fn add_property(&mut self, name: &str, value: &str) -> Result<(), String> {
        match self
            .state
            .properties
            .iter_mut()
            .enumerate()
            .find(|(_, (key, _))| name == key)
        {
            Some((index, (value_mut, _))) => {
                match self.state.handle.properties.get(index) {
                    Some((_, accepted_values)) => {
                        let owned_value = value.to_owned();

                        // Make sure the value being added is valid
                        if accepted_values.contains(&owned_value) {
                            *value_mut = owned_value;
                            Ok(())
                        } else {
                            Err(format!(
                                "Invalid property value for {} in {}: {}",
                                name, self.state.handle.name, value
                            ))
                        }
                    }

                    None => Err(format!(
                        "Encountered corrupted state while building {}",
                        self.state.handle.name
                    )),
                }
            }

            None => Err(format!(
                "Invalid property for {}: {}",
                self.state.handle.name, name
            )),
        }
    }

    pub fn with_property(mut self, name: &str, value: &str) -> Result<Self, (Self, String)> {
        match self.add_property(name, value) {
            Ok(()) => Ok(self),
            Err(message) => Err((self, message)),
        }
    }

    pub fn with_property_unchecked(mut self, name: &str, value: &str) -> Self {
        self.state
            .properties
            .iter_mut()
            .find(|(key, v)| name == key)
            .unwrap()
            .1 = value.to_owned();
        self
    }

    pub fn build(self) -> DynamicBlockState {
        self.state
    }
}

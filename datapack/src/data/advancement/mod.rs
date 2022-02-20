use std::collections::HashMap;

use qdat::UnlocalizedName;

use serde::{Deserialize, Serialize};

pub mod conditions;
pub mod display;

use crate::data::advancement::conditions::AdvancementConditions;

use self::display::AdvancementDisplay;

/// An advancement
#[derive(Serialize, Deserialize)]
pub struct Advancement {
    pub display: Option<AdvancementDisplay>,
    pub parent: Option<UnlocalizedName>,
    pub criteria: HashMap<String, AdvancementConditions>,
    pub requirements: Option<AdvancementRequirements>,
    pub rewards: Option<AdvancementRewards>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum AdvancementRequirements {
    /// A list of the required criteria
    List(Vec<String>),
    /// A list of lists of criteria
    ///
    /// All of the lists only have to have one of their criteria met
    ///
    /// Basically ANDing of OR groups
    LogicalList(Vec<Vec<String>>),
}

/// The rewards of an [Advancement]
#[derive(Serialize, Deserialize)]
pub struct AdvancementRewards {
    pub recipes: Option<Vec<UnlocalizedName>>,
    pub loot: Option<Vec<UnlocalizedName>>,
    pub experience: Option<i32>,
    pub function: Option<String>,
}

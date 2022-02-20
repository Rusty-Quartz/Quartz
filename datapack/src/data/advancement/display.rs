use qdat::UnlocalizedName;
use quartz_chat::Component;
use serde::{Deserialize, Serialize};

/// The display element of an [Advancement]
#[derive(Serialize, Deserialize)]
pub struct AdvancementDisplay {
    pub icon: Option<AdvancementIcon>,
    // TODO: this is a text component
    pub title: AdvancementDisplayText,
    pub frame: Option<AdvancementFrame>,
    pub background: Option<String>,
    // Same as title
    pub description: AdvancementDisplayText,
    pub show_toast: bool,
    pub announce_to_chat: bool,
    pub hidden: bool,
}

/// The icon of an [AdvancementDisplay]
#[derive(Serialize, Deserialize)]
pub struct AdvancementIcon {
    pub item: UnlocalizedName,
    // This is actual snbt supposidly? idk I'll deal with it later
    // TODO: make this actual nbt
    pub nbt: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub enum AdvancementFrame {
    #[serde(rename = "task")]
    Task,
    #[serde(rename = "challenge")]
    Challenge,
    #[serde(rename = "goal")]
    Goal,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum AdvancementDisplayText {
    String(String),
    TextComponent(Component),
    TextComponentList(Vec<Component>),
    // WHY THE FUCK IS THIS VALID
    Number(f32),
    Boolean(bool),
}

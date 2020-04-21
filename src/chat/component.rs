use std::fmt;

use serde::{Serialize, Deserialize};
use serde_json;
use serde_with::skip_serializing_none;

use crate::nbt::NbtCompound;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Color {
    Black,
    DarkBlue,
    DarkGreen,
    DarkAqua,
    DarkRed,
    DarkPurple,
    Gold,
    Gray,
    DarkGray,
    Blue,
    Green,
    Aqua,
    Red,
    LightPurple,
    Yellow,
    White,
    Reset
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Obfuscated(bool),
    Bold(bool),
    Strikethrough(bool),
    Underline(bool),
    Italic(bool)
}

#[repr(transparent)]
pub struct Message(Vec<Component>);

#[skip_serializing_none]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Component {
    // Main fields
    text: String,
    color: Option<Color>,
    
    // Formatting
    obfuscated: Option<bool>,
    bold: Option<bool>,
    strikethrough: Option<bool>,
    underline: Option<bool>,
    italic: Option<bool>,

    // Less often used
    insertion: Option<String>,
    click_event: Option<Box<ClickEvent>>,
    hover_event: Option<Box<HoverEvent>>,
    extra: Option<Vec<Component>>
}

impl Component {
    pub fn set_click_event(&mut self, event: ClickEvent) {
        self.click_event = Some(Box::new(event))
    }
}

impl fmt::Display for Component {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match serde_json::to_string(self) {
            Ok(string) => write!(f, "{}", string),
            Err(_) => write!(f, "{{}}")
        }
    }
}

#[derive(Serialize, Deserialize)]
struct RawEvent {
    action: String,
    value: EventArgument
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum EventArgument {
    Component(Component),
    Text(String),
    Index(u32)
}

#[derive(Serialize, Deserialize)]
pub struct ClickEvent {
    action: String,
    value: EventArgument
}

impl ClickEvent {
    pub fn open_url(value: String) -> Self {
        ClickEvent {
            action: "open_url".to_owned(),
            value: EventArgument::Text(value)
        }
    }

    pub fn run_command(value: String) -> Self {
        ClickEvent {
            action: "run_command".to_owned(),
            value: EventArgument::Text(value)
        }
    }

    pub fn suggest_command(value: String) -> Self {
        ClickEvent {
            action: "suggest_command".to_owned(),
            value: EventArgument::Text(value)
        }
    }

    pub fn page_change(value: u32) -> Self {
        ClickEvent {
            action: "page_change".to_owned(),
            value: EventArgument::Index(value)
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct HoverEvent {
    action: String,
    value: EventArgument
}

impl HoverEvent {
    pub fn show_text(value: String) -> Self {
        HoverEvent {
            action: "show_text".to_owned(),
            value: EventArgument::Text(value)
        }
    }

    pub fn show_text_component(value: Component) -> Self {
        HoverEvent {
            action: "show_text".to_owned(),
            value: EventArgument::Component(value)
        }
    }

    pub fn show_item(value: NbtCompound) -> Self {
        HoverEvent {
            action: "show_item".to_owned(),
            value: EventArgument::Text(value.to_string())
        }
    }

    pub fn show_entity(value: NbtCompound) -> Self {
        HoverEvent {
            action: "show_entity".to_owned(),
            value: EventArgument::Text(value.to_string())
        }
    }
}
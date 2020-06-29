use std::fmt;
use std::str;

use serde::{Serialize, Deserialize};
use serde_json::{self, error::Result as SerdeResult};
use serde_with::skip_serializing_none;

#[cfg(unix)]
use termion::style;

use crate::color::{Color, PredefinedColor};

// The generalized component type, including text, translate, selector, keybind, and nbt components
#[skip_serializing_none]
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum Component {
    Text(TextComponent),
    Translate {
        translate: String,
        with: Option<Vec<Component>>
    },
    Selector {
        selector: String
    },
    Keybind {
        keybind: String
    },
    Nbt {
        nbt: String,
        interpret: Option<bool>,
        block: Option<String>,
        entity: Option<String>,
        storage: Option<String>
    }
}

impl Component {
    pub fn text(text: String) -> Self {
        Component::Text(TextComponent::new(text, None))
    }

    pub fn colored(text: String, color: PredefinedColor) -> Self {
        Component::Text(TextComponent::new(text, Some(Color::Predefined(color))))
    }

    pub fn from_json(json: &str) -> SerdeResult<Self> {
        serde_json::from_str(json)
    }

    pub fn as_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    pub fn as_plain_text(&self) -> String {
        match self {
            Component::Text(text_component) => text_component.as_plain_text(),
            // TODO: Implement this for other component types
            _ => self.as_json()
        }
    }
}

impl Default for Component {
    fn default() -> Self {
        Component::Text(TextComponent::new(String::new(), None))
    }
}

impl fmt::Display for Component {
    // Display the component
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            // Handled by the text component struct
            Component::Text(inner) => inner.fmt(f),
            // TODO: implement the translate component
            _ => {
                match serde_json::to_string(self) {
                    Ok(string) => write!(f, "{}", string),
                    Err(_) => write!(f, "{{}}")
                }
            }
        }
    }
}

// For ease of use, this was moved outside of the component enum as it is significantly
// more complicated to handle
#[skip_serializing_none]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextComponent {
    pub text: String,
    pub color: Option<Color>,
    pub obfuscated: Option<bool>,
    pub bold: Option<bool>,
    pub strikethrough: Option<bool>,
    pub underline: Option<bool>,
    pub italic: Option<bool>,
    pub insertion: Option<String>,
    pub click_event: Option<Box<ClickEvent>>,
    pub hover_event: Option<Box<HoverEvent>>,
    pub extra: Option<Vec<Component>>
}

impl TextComponent {
    pub fn new(text: String, color: Option<Color>) -> Self {
        TextComponent {
            text,
            color,
            obfuscated: None,
            bold: None,
            strikethrough: None,
            underline: None,
            italic: None,
            insertion: None,
            click_event: None,
            hover_event: None,
            extra: None
        }
    }

    // Only copies color and formats
    pub fn copy_formatting(text: String, component: &TextComponent) -> Self {
        TextComponent {
            text,
            color: component.color,
            obfuscated: component.obfuscated,
            bold: component.bold,
            strikethrough: component.strikethrough,
            underline: component.underline,
            italic: component.italic,
            insertion: None,
            click_event: None,
            hover_event: None,
            extra: None
        }
    }

    // Returns if the text is empty
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn as_plain_text(&self) -> String {
        let mut text = self.text.clone();

        // Append children's text
        if let Some(children) = &self.extra {
            for child in children.iter() {
                text.push_str(&child.as_plain_text());
            }
        }

        text
    }

    // Adds the given child, creating the children array if needed
    pub fn add_child(&mut self, component: Component) {
        match &mut self.extra {
            Some(children) => children.push(component),
            None => self.extra = Some(vec![component])
        }
    }

    pub fn has_children(&self) -> bool {
        self.extra.is_some() && !self.extra.as_ref().unwrap().is_empty()
    }

    // Apply just the formatting of this component
    #[cfg(unix)]
    fn apply_format(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Apply the color
        if let Some(color) = &self.color {
            color.apply(f)?;
        }

        // Apply formatting variables
        #[cfg(unix)]
        macro_rules! apply_format {
            ($field:expr, $add_style:ident, $remove_style:ident) => {
                if let Some(value) = $field {
                    if value {
                        write!(f, "{}", style::$add_style)?;
                    } else {
                        write!(f, "{}", style::$remove_style)?;
                    }
                }
            };
        }

        apply_format!(self.bold, Bold, NoBold);
        apply_format!(self.strikethrough, CrossedOut, NoCrossedOut);
        apply_format!(self.underline, Underline, NoUnderline);
        apply_format!(self.italic, Italic, NoItalic);

        Ok(())
    }
}

impl fmt::Display for TextComponent {
    #[cfg(unix)]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Gnome supports hyperlinks, so if we have a link as the click event, apply it
        let mut link_applied = false;
        if let Some(event) = &self.click_event {
            if event.action == "open_url" {
                if let EventArgument::Text(url) = &event.value {
                    write!(f, "\x1B]8;;{}\x1B\\", url)?;
                    link_applied = true;
                }
            }
        }

        self.apply_format(f)?;

        // Write the text
        write!(f, "{}", self.text)?;

        // Write the children
        if let Some(children) = &self.extra {
            for child in children.iter() {
                child.fmt(f)?;

                // A bit redundant but gets the job done
                self.apply_format(f)?;
            }
        }

        // Close off the hyperlink syntax if needed
        if link_applied {
            write!(f, "\x1B]8;;\x1B\\")?;
        }

        // Undo the changes
        PredefinedColor::Reset.apply(f)?;

        Ok(())
    }

    #[cfg(not(unix))]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}", self.text)?;
		if let Some(children) = &self.extra {
			for child in children.iter() {
	    		child.fmt(f)?;
	  		}
		}
		Ok(())
    }
}

// Click event type for text components
#[derive(Serialize, Deserialize)]
pub struct ClickEvent {
    action: String,
    value: EventArgument
}

impl ClickEvent {
    pub fn open_url(url: String) -> Self {
        ClickEvent {
            action: "open_url".to_owned(),
            value: EventArgument::Text(url)
        }
    }

    pub fn run_command(command: String) -> Self {
        ClickEvent {
            action: "run_command".to_owned(),
            value: EventArgument::Text(command)
        }
    }

    pub fn suggest_command(command: String) -> Self {
        ClickEvent {
            action: "suggest_command".to_owned(),
            value: EventArgument::Text(command)
        }
    }

    pub fn change_page(index: u32) -> Self {
        ClickEvent {
            action: "change_page".to_owned(),
            value: EventArgument::Index(index)
        }
    }
}

// Hover event type for text components
#[skip_serializing_none]
#[derive(Serialize, Deserialize)]
pub struct HoverEvent {
    action: String,
    contents: Option<HoverContents>,
    // This is for legacy support
    value: Option<EventArgument>
}

impl HoverEvent {
    pub fn show_text(text: TextComponent) -> Self {
        HoverEvent {
            action: "show_text".to_owned(),
            contents: Some(HoverContents::Component(Component::Text(text))),
            value: None
        }
    }

    pub fn show_item(json: &str) -> Option<Self> {
        let contents: HoverContents;

        // Try to parse the json
        match serde_json::from_str::<HoverContents>(json) {
            Ok(parsed) => {
                // Ensure that it matches the item type
                match parsed {
                    HoverContents::Item {id, count, tag} => contents = HoverContents::Item {id, count, tag},
                    _ => return None
                }
            },
            // Assume just the item ID was passed in
            Err(_) => contents = HoverContents::ItemId(json.to_owned())
        }

        Some(HoverEvent {
            action: "show_item".to_owned(),
            contents: Some(contents),
            value: None
        })
    }

    pub fn show_entity(json: &str) -> Option<Self> {
        // Try to parse the json
        match serde_json::from_str::<HoverContents>(json) {
            Ok(parsed) => {
                // Ensure it matches the entity type
                match parsed {
                    HoverContents::Entity {id, name, entity_type} => {
                        Some(HoverEvent {
                            action: "show_entity".to_owned(),
                            contents: Some(HoverContents::Entity {id, name, entity_type}),
                            value: None
                        })
                    },
                    _ => return None
                }
            },
            Err(_) => return None
        }
    }
}

// The contents variable in the hover event
#[skip_serializing_none]
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum HoverContents {
    Component(Component),
    ItemId(String),
    Item {
        id: String,
        count: u8,
        tag: Option<String>
    },
    Entity {
        id: String,
        name: Option<Component>,
        #[serde(rename = "type")]
        entity_type: Option<String>
    }
}

// The generalized event argument
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum EventArgument {
    Component(Component),
    Text(String),
    Index(u32)
}
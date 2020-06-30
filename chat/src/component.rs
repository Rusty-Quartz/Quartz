use std::fmt;
use std::str;

use serde::{Serialize, Deserialize};
use serde_with::skip_serializing_none;

#[cfg(unix)]
use termion::style;

use crate::color::{Color, PredefinedColor};

/// The generalized component type, including: text, translate, selector, keybind, and nbt components.
#[skip_serializing_none]
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum Component {
    /// A text component.
    Text(TextComponent),
    /// A translate component which requires values to be inserted into a predefined format.
    Translate {
        /// The unlocalized translation ID to use for this component.
        translate: String,
        /// The components to insert into the translation.
        with: Option<Vec<Component>>
    },
    /// A component consisting of an entity selector, such as `@a` or `@e[distance=..3]`.
    Selector {
        /// The selector in string-form.
        selector: String
    },
    /// Used to display the client's current keybind for the specified key.
    Keybind {
        /// They key whose binding should be specified.
        keybind: String
    }

    // TODO: Add score components
}

impl Component {
    /// Creates a text component with the given text and no color.
    pub fn text(text: String) -> Self {
        Component::Text(TextComponent::new(text, None))
    }

    /// Creates a component with the given text and predefined color.
    pub fn colored(text: String, color: PredefinedColor) -> Self {
        Component::Text(TextComponent::new(text, Some(Color::Predefined(color))))
    }

    /// Converts this component into plain, uncolored text.
    pub fn as_plain_text(&self) -> String {
        match self {
            Component::Text(text_component) => text_component.as_plain_text(),
            // TODO: Implement this for other component types
            _ => serde_json::to_string(self).unwrap_or("{}".to_owned())
        }
    }
}

impl From<TextComponent> for Component {
    fn from(text_component: TextComponent) -> Self {
        Component::Text(text_component)
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

/// A component with text, color, formatting, click/hover events, etc.
#[skip_serializing_none]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextComponent {
    /// The raw text in the component.
    pub text: String,
    /// The color of the component.
    pub color: Option<Color>,
    /// Whether or not the component should be obfuscated.
    pub obfuscated: Option<bool>,
    /// Whether or not the component should be bolded.
    pub bold: Option<bool>,
    /// Whether or not the component should be struck-through.
    pub strikethrough: Option<bool>,
    /// Whether or not the component should be underlined.
    pub underline: Option<bool>,
    /// Whether or not the component should be italicized.
    pub italic: Option<bool>,
    /// The text to insert into a player's chat upon shift-clicking this component.
    pub insertion: Option<String>,
    /// The event to run when this component is clicked.
    pub click_event: Option<Box<ClickEvent>>,
    /// The event to run when the player hovers over this component.
    pub hover_event: Option<Box<HoverEvent>>,
    /// The children of this component.
    pub extra: Option<Vec<Component>>
}

impl TextComponent {
    /// Creates a new text component with the given text and color.
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

    /// Creates a text component with the given text, copying the color and formatting of the given component.
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

    /// Returns whether or not the text is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Converts this component into plain text by concatenating this component's text field with its children's text
    /// fields.
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

    /// Adds the given child, creating the children vec if needed.
    pub fn add_child(&mut self, component: Component) {
        match &mut self.extra {
            Some(children) => children.push(component),
            None => self.extra = Some(vec![component])
        }
    }

    /// Returns whether or not this component has children.
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
            if event.action == ClickEventType::OpenUrl {
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

/// Defines click events for text components.
#[derive(Serialize, Deserialize)]
pub struct ClickEvent {
    action: ClickEventType,
    value: EventArgument
}

impl ClickEvent {
    /// Creates a click event which prompts the client to go to the given URL.
    pub fn open_url(url: String) -> Self {
        ClickEvent {
            action: ClickEventType::OpenUrl,
            value: EventArgument::Text(url)
        }
    }

    /// Creates a click event which runs the given command with the clicker as the sender.
    pub fn run_command(command: String) -> Self {
        ClickEvent {
            action: ClickEventType::RunCommand,
            value: EventArgument::Text(command)
        }
    }

    /// Creates a click event which suggests the given command to the clicker.
    pub fn suggest_command(command: String) -> Self {
        ClickEvent {
            action: ClickEventType::SuggestCommand,
            value: EventArgument::Text(command)
        }
    }

    /// Creates a click event which changes a client's page while reading a book.
    pub fn change_page(index: u32) -> Self {
        ClickEvent {
            action: ClickEventType::ChangePage,
            value: EventArgument::Index(index)
        }
    }
}

#[derive(PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ClickEventType {
    OpenUrl,
    RunCommand,
    SuggestCommand,
    ChangePage
}

/// Defines hover events for text components.
#[skip_serializing_none]
#[derive(Serialize, Deserialize)]
pub struct HoverEvent {
    action: HoverEventType,
    contents: Option<HoverContents>,
    // This is for legacy support
    value: Option<EventArgument>
}

impl HoverEvent {
    /// Creates a hover event which will display the given component.
    pub fn show_text(component: Component) -> Self {
        HoverEvent {
            action: HoverEventType::ShowText,
            contents: Some(HoverContents::Component(component)),
            value: None
        }
    }

    /// Creates a hover event which will display the given item.
    pub fn show_item(item: HoverItem) -> Self {
        HoverEvent {
            action: HoverEventType::ShowItem,
            contents: Some(HoverContents::Item(item)),
            value: None
        }
    }

    /// Attempts to parse the given JSON into an item profile and create a hover event to display
    /// the item as defined by the JSON. If the parsing fails, then the JSON string will be treated as
    /// a raw item ID instead.
    pub fn show_item_json(json: &str) -> Self {
        let contents: HoverContents;

        // Try to parse the json
        match serde_json::from_str::<HoverItem>(json) {
            Ok(parsed) => contents = HoverContents::Item(parsed),
            // Assume just the item ID was passed in
            Err(_) => contents = HoverContents::ItemId(json.to_owned())
        }

        HoverEvent {
            action: HoverEventType::ShowItem,
            contents: Some(contents),
            value: None
        }
    }

    /// Creates a hover event which will display the given entity.
    pub fn show_entity(entity: HoverEntity) -> Self {
        HoverEvent {
            action: HoverEventType::ShowEntity,
            contents: Some(HoverContents::Entity(entity)),
            value: None
        }
    }

    /// Attempts to parse the given JSON into an entity profile and create a hover event to display
    /// the entity as defined by the JSON.
    pub fn show_entity_json(json: &str) -> Option<Self> {
        // Try to parse the json
        match serde_json::from_str::<HoverEntity>(json) {
            Ok(parsed) => {
                Some(HoverEvent {
                    action: HoverEventType::ShowEntity,
                    contents: Some(HoverContents::Entity(parsed)),
                    value: None
                })
            },
            Err(_) => None
        }
    }
}

#[derive(PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum HoverEventType {
    ShowText,
    ShowItem,
    ShowEntity
}

// The contents variable in the hover event
#[skip_serializing_none]
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum HoverContents {
    Component(Component),
    ItemId(String),
    Item(HoverItem),
    Entity(HoverEntity)
}

/// Defines an item profile which can be displayed through hover events.
#[skip_serializing_none]
#[derive(Serialize, Deserialize)]
pub struct HoverItem {
    id: String,
    count: u8,
    tag: Option<String>
}

/// Defines an entity profile which can be displayed through hover events.
#[skip_serializing_none]
#[derive(Serialize, Deserialize)]
pub struct HoverEntity {
    id: String,
    name: Option<Component>,
    #[serde(rename = "type")]
    entity_type: Option<String>
}

// The generalized event argument
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum EventArgument {
    Component(Component),
    Text(String),
    Index(u32)
}
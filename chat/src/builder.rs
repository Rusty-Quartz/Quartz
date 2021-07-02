use crate::{
    color::{Color, PredefinedColor},
    component::{ClickEvent, Component, ComponentType, HoverEvent},
};

/// Utility struct for building text components. Components can have children, and those children
/// can have children, etc., however this utility only allows for a base component with a list of
/// standalone child components. A new child with different formatting can be appended via the `add`
/// method, and the same methods for adjusting color and formatting can be used to modify that child
/// component.
pub struct ComponentBuilder {
    component: Component,
    current_empty: bool,
}

macro_rules! component_format {
    ($name:ident, $comment:expr) => {
        #[doc = concat!("Set whether or not this component's text should be ", $comment, ".")]
        pub fn $name(mut self, value: bool) -> Self {
            self.current().$name = Some(value);
            self
        }
    };
}

impl ComponentBuilder {
    component_format!(
        obfuscated,
        "obfuscated (quickly changing, fixed-width text)"
    );

    component_format!(bold, "bolded");

    component_format!(strikethrough, "struck-through");

    component_format!(underline, "underlined");

    component_format!(italic, "italicized");

    /// Creates a builder whose base component has no color and an empty text field.
    pub const fn empty() -> Self {
        ComponentBuilder {
            component: Component::empty(),
            current_empty: false,
        }
    }

    /// Creates a builder whose base component has no text and the color white.
    pub fn new() -> Self {
        ComponentBuilder {
            component: Component::colored(String::new(), PredefinedColor::White),
            current_empty: false,
        }
    }

    /// Retrieves the current component being built.
    fn current(&mut self) -> &mut Component {
        self.current_empty = false;

        if self.component.has_children() {
            // These unwraps are checked above
            self.component.extra.as_mut().unwrap().last_mut().unwrap()
        } else {
            &mut self.component
        }
    }

    /// Finish the current component and prepare a new component which can have a different color, formatting, etc.
    pub fn add(mut self, component_type: ComponentType) -> Self {
        self.current().component_type = component_type;
        self.add_empty()
    }

    /// Finish the current component, setting its text field to the given value, and prepare a new component
    /// which can have a different color, formatting, etc.
    pub fn add_text<T: ToString>(self, text: T) -> Self {
        self.add(ComponentType::text(text))
    }

    /// Finish the current component, setting its text field to an empty string, and prepare a new component
    /// which can have a different color, formatting, etc.
    pub fn add_empty(mut self) -> Self {
        if !self.current_empty {
            self.component.add_child(Component::empty());
            self.current_empty = true;
        }

        self
    }

    /// Consumes this builder and returns a finished text component.
    pub fn build(mut self) -> Component {
        if self.current_empty {
            let _ = self.component.extra.as_mut().unwrap().pop();
        }

        self.component
    }

    /// Consumes this builder and returns a vec of the base component's children, excluding the base
    /// component itself.
    pub fn build_children(mut self) -> Vec<Component> {
        if self.current_empty {
            let _ = self.component.extra.as_mut().unwrap().pop();
        }

        match self.component.extra {
            Some(children) => children,
            None => Vec::new(),
        }
    }

    /// Set the color of the current component to the given color.
    pub fn color<C: Into<Color>>(mut self, color: C) -> Self {
        self.current().color = Some(color.into());
        self
    }

    /// Set the color of the current component to a custom color.
    pub fn custom_color(mut self, red: u8, green: u8, blue: u8) -> Self {
        self.current().color = Some(Color::Custom(red, green, blue));
        self
    }

    /// Set the font of this component.
    pub fn font(mut self, font: String) -> Self {
        self.current().font = Some(font);
        self
    }

    /// Set the insertion text of this component (the text inserted into a player's chat when
    /// they shift-click this component).
    pub fn insertion(mut self, insertion: String) -> Self {
        self.current().insertion = Some(insertion);
        self
    }

    /// Set the click event of this component.
    pub fn click_event(mut self, event: ClickEvent) -> Self {
        self.current().click_event = Some(Box::new(event));
        self
    }

    /// Set the hover event of this component.
    pub fn hover_event(mut self, event: HoverEvent) -> Self {
        self.current().hover_event = Some(Box::new(event));
        self
    }
}

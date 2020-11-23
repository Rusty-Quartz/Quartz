use crate::{
    color::{Color, PredefinedColor},
    component::{ClickEvent, Component, HoverEvent, TextComponent},
};
use doc_comment::doc_comment;

/// Utility struct for building text components. Components can have children, and those children
/// can have children, etc., however this utility only allows for a base component with a list of
/// standalone child components. A new child with different formatting can be appended via the `add`
/// method, and the same methods for adjusting color and formatting can be used to modify that child
/// component.
pub struct TextComponentBuilder {
    component: TextComponent,
    current_empty: bool,
}

macro_rules! component_format {
    ($name:ident, $comment:expr) => {
        doc_comment! {
            concat!("Set whether or not this component's text should be ", $comment, "."),
            pub fn $name(mut self, value: bool) -> Self {
                self.current().$name = Some(value);
                self
            }
        }
    };
}

impl TextComponentBuilder {
    component_format!(obfuscated, "obfuscated (quickly chaning, fixed-width text)");

    component_format!(bold, "bolded");

    component_format!(strikethrough, "struck-through");

    component_format!(underline, "underlined");

    component_format!(italic, "italicized");

    /// Creates a builder whose base component has no color and an empty text field.
    pub fn empty() -> Self {
        TextComponentBuilder {
            component: TextComponent::new(String::new(), None),
            current_empty: false,
        }
    }

    /// Creates a builder whose base component has the given text and no color.
    pub fn new(text: String) -> Self {
        TextComponentBuilder {
            component: TextComponent::new(text.to_owned(), None),
            current_empty: false,
        }
    }

    /// Retrieves the current component being built.
    fn current(&mut self) -> &mut TextComponent {
        self.current_empty = false;

        if self.component.has_children() {
            // This unwrap is checked above
            match self.component.extra.as_mut().unwrap().last_mut() {
                Some(Component::Text(component)) => component,
                // TODO: replace with &mut self.component when the borrow checker allows it in the future
                _ => unreachable!(),
            }
        } else {
            &mut self.component
        }
    }

    /// Finish the current component and prep a new component which can have different color, formatting, etc.
    pub fn add(mut self) -> Self {
        if !self.current_empty {
            self.component
                .add_child(Component::Text(TextComponent::new(String::new(), None)));
            self.current_empty = true;
        }

        self
    }

    /// Comsumes this builder and returns a finished text component.
    pub fn build<C: From<TextComponent>>(mut self) -> C {
        if self.current_empty {
            let _ = self.component.extra.as_mut().unwrap().pop();
        }

        self.component.into()
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

    /// Set the text of the current component.
    pub fn text(mut self, text: String) -> Self {
        self.current().text = text.to_owned();
        self
    }

    /// Set the color of the current component to a predefined color.
    pub fn predef_color(mut self, color: PredefinedColor) -> Self {
        self.current().color = Some(color.into());
        self
    }

    /// Set the color of the current component to a custom color.
    pub fn custom_color(mut self, red: u8, green: u8, blue: u8) -> Self {
        self.current().color = Some(Color::Custom(red, green, blue));
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

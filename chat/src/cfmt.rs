use crate::{color::Color, component::*};
use std::{
    error::Error,
    fmt::{self, Debug, Display, Formatter},
};

/// Wrapper for an error message related to CFMT parsing.
pub struct CfmtError(String);

impl Display for CfmtError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Debug for CfmtError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl Error for CfmtError {}

#[inline]
fn try_unwrap<T>(x: Option<T>, message: &str) -> Result<T, CfmtError> {
    x.ok_or(CfmtError(format!("Internal parser error: {}", message)))
}

const INDEX_STACK_ERROR: &str = "index stack empty";
const TOKEN_STACK_ERROR: &str = "token stack empty";
const COMPONENT_STACK_ERROR: &str = "component stack empty";
const CHILD_STACK_ERROR: &str = "children stack empty";

/// Parses the given string slice into a text component.
pub fn parse_cfmt(cfmt: &str) -> Result<Component, CfmtError> {
    let mut stack: Vec<TextComponent> = Vec::with_capacity(4);
    stack.push(TextComponent::new(String::new(), None));

    // Variables that need to last between characters and aren't reflective of state
    // Current character index
    let mut index: usize = 0;

    // Current token being built
    let mut token_stack: Vec<String> = Vec::with_capacity(4);

    // Whether or not a formatting code was negated
    let mut apply_format: bool = true;

    // Keep track of bracket pairs
    let mut curly_bracket_depth: usize = 0;

    // Whether or not the component at the current depth according to the variable above
    // has any children appended to it
    let mut has_children: Vec<bool> = vec![false];

    // Keep track of indices where errors could occur
    let mut index_stack: Vec<usize> = Vec::with_capacity(8);

    const NORMAL: u8 = 0;
    const EVENT: u8 = 1;
    let mut component_type: u8 = NORMAL;

    // Parser state
    const ADD_TEXT: u8 = 0;
    const FORCE_ADD: u8 = 1;
    const COLOR_START: u8 = 2;
    const COLOR_BUILD_FIRST: u8 = 3;
    const COLOR_BUILD_EXTRA: u8 = 4;
    const EVENT_START: u8 = 5;
    const EVENT_BUILD_TYPE: u8 = 6;
    const EVENT_BUILD_NAME: u8 = 7;
    const EVENT_BUILD_ARG: u8 = 8;
    let mut state = ADD_TEXT;

    // Return an error formatted to the given format
    macro_rules! error {
        ($format:expr, $len:expr) => {{
            let idx = *try_unwrap(index_stack.last(), INDEX_STACK_ERROR)?;

            let mut right = idx + $len.min(35);
            if right > cfmt.len() {
                right = cfmt.len();
            }

            return Err(CfmtError(format!($format, &cfmt[idx .. right])));
        }};
        ($msg:expr) => {{
            return Err(CfmtError($msg.to_owned()));
        }};
    }

    // Mark the current index as a potential error site
    macro_rules! mark {
        ($offset:expr) => {
            index_stack.push(index + $offset);
        };
        () => {
            index_stack.push(index);
        };
    }

    // Pop the last index of the index stack
    macro_rules! unmark {
        () => {{
            let _ = index_stack.pop();
        }};
    }

    macro_rules! current_token {
        () => {
            (try_unwrap(token_stack.last_mut(), TOKEN_STACK_ERROR)?)
        };
    }

    // Take the top component off the stack and add it to the next component on the stack
    macro_rules! collapse {
        () => {{
            let child = try_unwrap(stack.pop(), COMPONENT_STACK_ERROR)?;
            if !child.is_empty() {
                add_child(try_unwrap(stack.last_mut(), COMPONENT_STACK_ERROR)?, child)?;
            }
        }};
    }

    for ch in cfmt.chars() {
        match state {
            ADD_TEXT => {
                match ch {
                    '\\' => state = FORCE_ADD,

                    '&' => {
                        finish_component(&mut stack, &mut has_children)?;

                        state = COLOR_START;
                        token_stack.push(String::with_capacity(8));

                        // Mark the start of the sequence
                        mark!();
                    }

                    '{' => {
                        // Manage depth
                        curly_bracket_depth += 1;

                        // Keep the current component on the stack for reference when determining the format of the
                        // component after this block
                        let next;
                        if *try_unwrap(has_children.last(), CHILD_STACK_ERROR)? {
                            next = TextComponent::copy_formatting(
                                String::new(),
                                try_unwrap(stack.last(), COMPONENT_STACK_ERROR)?,
                            );
                        } else {
                            next = TextComponent::new(String::new(), None);
                        }
                        stack.push(next);

                        // Allows the appended component to have children
                        has_children.push(false);

                        // Used to identify unpaired curly braces
                        mark!();
                    }

                    '}' => {
                        // Somone has a random close curly bracket lying around
                        if curly_bracket_depth == 0 {
                            mark!();
                            error!("Unpaired curly bracket: \"{}...\"", 10);
                        }

                        curly_bracket_depth -= 1;

                        // If children were appended in the block, append the current one before the block is closed
                        if try_unwrap(has_children.pop(), CHILD_STACK_ERROR)? {
                            collapse!();
                        }

                        // The component representing the current block
                        let block = try_unwrap(stack.pop(), COMPONENT_STACK_ERROR)?;
                        // The component to attach the block to
                        let last: &mut TextComponent;
                        // The next component to push on the stack
                        let next: TextComponent;

                        // The outer component to attach this block to has children
                        if *try_unwrap(has_children.last(), CHILD_STACK_ERROR)? {
                            // Grab the component we left behind earlier to get the format for the next component
                            let reference = try_unwrap(stack.pop(), COMPONENT_STACK_ERROR)?;

                            // Copy formatting from the reference
                            next = TextComponent::copy_formatting(String::new(), &reference);

                            // Append the component we left behind
                            last = try_unwrap(stack.last_mut(), COMPONENT_STACK_ERROR)?;
                            add_child(last, reference)?;
                        }
                        // The outer component does not have any children, we are the first child
                        else {
                            // Formatting inherited from the outer block
                            next = TextComponent::new(String::new(), None);

                            last = try_unwrap(stack.last_mut(), COMPONENT_STACK_ERROR)?;

                            // Now the outer component will have children
                            *try_unwrap(has_children.last_mut(), CHILD_STACK_ERROR)? = true;
                        }

                        // Append this block
                        add_child(last, block)?;
                        stack.push(next);
                    }

                    '$' => {
                        // Don't allow nesting of events
                        if component_type == EVENT {
                            error!("Events cannot be nested within components attached to events.");
                        }

                        finish_component(&mut stack, &mut has_children)?;

                        state = EVENT_START;

                        // Mark the start of the event sequence
                        mark!();
                    }

                    ')' => {
                        // This only happens for event components
                        if component_type == EVENT {
                            finish_event(&mut stack, &mut has_children, &mut token_stack, true)?;

                            state = ADD_TEXT;
                            component_type = NORMAL;

                            unmark!();
                        } else {
                            try_unwrap(stack.last_mut(), COMPONENT_STACK_ERROR)?
                                .text
                                .push(ch);
                        }
                    }

                    _ => try_unwrap(stack.last_mut(), COMPONENT_STACK_ERROR)?
                        .text
                        .push(ch),
                }
            }

            FORCE_ADD => {
                try_unwrap(stack.last_mut(), COMPONENT_STACK_ERROR)?
                    .text
                    .push(ch);
                state = ADD_TEXT;
            }

            COLOR_START => {
                if ch == '(' {
                    state = COLOR_BUILD_FIRST;
                    current_token!().push('\"'); // For serde compatability

                    // Mark the start of the first color/format
                    mark!(1);
                } else {
                    error!("Expected open parenthesis after '&': \"{}...\"", 10);
                }
            }

            COLOR_BUILD_FIRST | COLOR_BUILD_EXTRA => {
                match ch {
                    ',' | ')' => {
                        let mut token = try_unwrap(token_stack.pop(), TOKEN_STACK_ERROR)?;

                        // Check for dangling comma
                        if ch == ')' && token.len() == 1 {
                            // Remove mark for the current color/format
                            unmark!();

                            error!(
                                "Dangling comma at the end of formatting sequence: \"{}\"",
                                index - try_unwrap(index_stack.last(), INDEX_STACK_ERROR)? + 1
                            );
                        }

                        token.push('\"');
                        let mut component = try_unwrap(stack.last_mut(), COMPONENT_STACK_ERROR)?;

                        // Match the current item to a color
                        if state == COLOR_BUILD_FIRST {
                            if let Ok(color) = serde_json::from_str::<Color>(&token) {
                                if !apply_format {
                                    error!(
                                        "Negation character not allowed in front of a color code: \
                                         \"{}\"",
                                        index - try_unwrap(index_stack.last(), INDEX_STACK_ERROR)?
                                    );
                                }

                                component.color = Some(color);
                                token.clear();
                            }

                            state = COLOR_BUILD_EXTRA;
                        }

                        // Match the current item to a formatting code
                        if !token.is_empty() {
                            match token.as_ref() {
                                "\"obfuscated\"" => component.obfuscated = Some(apply_format),
                                "\"bold\"" => component.bold = Some(apply_format),
                                "\"strikethrough\"" => component.strikethrough = Some(apply_format),
                                "\"underline\"" => component.underline = Some(apply_format),
                                "\"italic\"" => component.italic = Some(apply_format),
                                _ => {
                                    // Check to see if they tried to add a color, which we enforce as the first element
                                    if serde_json::from_str::<Color>(&token).is_ok() {
                                        // Remove mark for the current color/format
                                        unmark!();

                                        error!(
                                            "Excpected color or \"reset\" as first argument of \
                                             color sequence: \"{}...\"",
                                            index
                                                - try_unwrap(
                                                    index_stack.last(),
                                                    INDEX_STACK_ERROR
                                                )?
                                        );
                                    }
                                    // The format or color name was incorrect
                                    else {
                                        error!(
                                            "Invalid color or formatting code: \"{}\"",
                                            index
                                                - try_unwrap(
                                                    index_stack.last(),
                                                    INDEX_STACK_ERROR
                                                )?
                                        );
                                    }
                                }
                            }

                            apply_format = true;
                        }

                        // Remove the mark for the current color/format
                        unmark!();

                        if ch == ',' {
                            // Mark the start of the next color/format
                            mark!(1);

                            token_stack.push(String::with_capacity(8));
                            current_token!().push('\"');
                        } else {
                            // Remove the mark at the start of the sequence
                            unmark!();

                            state = ADD_TEXT;
                        }
                    }

                    '!' => {
                        // Valid syntax, ex: !bold
                        if current_token!().len() == 1 {
                            apply_format = false;
                        }
                        // One exclamation in the middle of the word, probably a typo
                        else if apply_format {
                            error!(
                                "Expected negation character ('!') to be at the beginning of a \
                                 formatting code: \"{}...\"",
                                index - try_unwrap(index_stack.last(), INDEX_STACK_ERROR)? + 3
                            );
                        }
                        // Just pass this mess down to the format parser for the error
                        else {
                            current_token!().push(ch);
                        }
                    }

                    _ => current_token!().push(ch),
                }
            }

            EVENT_START => {
                // Enforce an open bracket at the start of the sequence
                if ch == '(' {
                    state = EVENT_BUILD_TYPE;
                    // Length five because "click" and "hover" are both 5 bytes
                    token_stack.push(String::with_capacity(5));

                    // Mark the start of the first color/format
                    mark!(1);
                } else {
                    error!("Expected open parenthesis after '$': \"{}...\"", 10);
                }
            }

            EVENT_BUILD_TYPE => {
                // Separate the event type from its name with a colon
                if ch == ':' {
                    let event_type: &str = current_token!();

                    // Valid event types
                    if event_type == "hover" || event_type == "click" {
                        token_stack.push(String::with_capacity(16));
                        state = EVENT_BUILD_NAME;

                        // Mark the start of the event name
                        unmark!();
                        mark!(1);
                    }
                    // Invalid type
                    else {
                        error!(
                            "Invalid event type, expected \"hover\" or \"click\" but found \"{}\"",
                            index - try_unwrap(index_stack.last(), INDEX_STACK_ERROR)?
                        );
                    }
                }
                // If it's not the delimeter, append the character to the token
                else {
                    current_token!().push(ch);
                }
            }

            EVENT_BUILD_NAME => {
                // Separate the event data from its argument witha comma
                if ch == ',' {
                    let event_type: &str = token_stack[token_stack.len() - 2].as_ref();
                    let event_name: &str = try_unwrap(token_stack.last(), TOKEN_STACK_ERROR)?;

                    match event_type {
                        "hover" => {
                            // Argument handling depends on the event name
                            match event_name {
                                // This one is weird because its argument is a component
                                "show_text" => {
                                    // Set everything up so we can still use this loop to parse the component
                                    state = ADD_TEXT;
                                    component_type = EVENT;
                                    stack.push(TextComponent::new(String::new(), None));
                                    has_children.push(false);
                                    continue;
                                }

                                // Both of these are JSON, so a string
                                "show_item" | "show_entity" => state = EVENT_BUILD_ARG,

                                // Invalid event name
                                _ => {
                                    error!(
                                        "Invalid event name for hover type: \"{}\"",
                                        index - try_unwrap(index_stack.last(), INDEX_STACK_ERROR)?
                                    );
                                }
                            }
                        }

                        "click" => {
                            // Valid click events
                            if event_name == "open_url"
                                || event_name == "run_command"
                                || event_name == "suggest_command"
                                || event_name == "change_page"
                            {
                                state = EVENT_BUILD_ARG;
                            }
                            // Invalid click event
                            else {
                                error!(
                                    "Invalid event name for click type: \"{}\"",
                                    index - try_unwrap(index_stack.last(), INDEX_STACK_ERROR)?
                                );
                            }
                        }

                        // This is checked beforehand
                        _ => {}
                    }

                    token_stack.push(String::with_capacity(16));

                    unmark!();
                    mark!(1);
                }
                // If it's not a comma, append to the argument token
                else {
                    current_token!().push(ch);
                }
            }

            EVENT_BUILD_ARG => {
                // Event sequence ends with a close parentehsis
                if ch == ')' {
                    finish_event(&mut stack, &mut has_children, &mut token_stack, false)?;

                    state = ADD_TEXT;

                    unmark!();
                } else {
                    current_token!().push(ch);
                }
            }

            _ => {}
        }

        index += 1;
    }

    match state {
        // Only valid state to reach the end in
        ADD_TEXT => {
            if component_type == EVENT {
                error!("Incomplete event sequence at the end of the input string.");
            }

            // Check to make sure all open brackets are matched
            if curly_bracket_depth > 0 {
                error!("Unpaired curly bracket: \"{}...\"", 10);
            }

            // Collapse the stack into a single component
            while stack.len() > 1 {
                collapse!();
            }

            return Ok(Component::Text(try_unwrap(
                stack.pop(),
                COMPONENT_STACK_ERROR,
            )?));
        }

        FORCE_ADD => {
            error!(
                "Expected another character after the escape character at the end of the input \
                 string."
            );
        }

        COLOR_START | COLOR_BUILD_FIRST | COLOR_BUILD_EXTRA => {
            error!("Incomplete color sequence at the end of the input string.");
        }

        EVENT_START | EVENT_BUILD_NAME | EVENT_BUILD_TYPE | EVENT_BUILD_ARG => {
            error!("Incomplete event sequence at the end of the input string.");
        }

        _ =>
            return Ok(Component::Text(try_unwrap(
                stack.pop(),
                COMPONENT_STACK_ERROR,
            )?)),
    }
}

fn finish_component(
    stack: &mut Vec<TextComponent>,
    has_children: &mut Vec<bool>,
) -> Result<(), CfmtError> {
    if !try_unwrap(stack.last(), COMPONENT_STACK_ERROR)?.is_empty() {
        // Some children are already present
        if *try_unwrap(has_children.last(), CHILD_STACK_ERROR)? {
            let child = try_unwrap(stack.pop(), COMPONENT_STACK_ERROR)?;

            // Manage inheritance here to prevent the JSON depth from getting insane
            let next = TextComponent::copy_formatting(String::new(), &child);

            add_child(try_unwrap(stack.last_mut(), COMPONENT_STACK_ERROR)?, child)?;
            stack.push(next);
        }
        // Add the first child
        else {
            stack.push(TextComponent::new(String::new(), None));
            *try_unwrap(has_children.last_mut(), CHILD_STACK_ERROR)? = true;
        }
    }

    Ok(())
}

fn finish_event(
    stack: &mut Vec<TextComponent>,
    has_children: &mut Vec<bool>,
    token_stack: &mut Vec<String>,
    has_component: bool,
) -> Result<(), CfmtError> {
    // Handle stack operations and retrieve the component argument
    if has_component {
        // Add any remaining child
        if try_unwrap(has_children.pop(), CHILD_STACK_ERROR)? {
            let child = try_unwrap(stack.pop(), COMPONENT_STACK_ERROR)?;
            add_child(try_unwrap(stack.last_mut(), COMPONENT_STACK_ERROR)?, child)?;
        }

        // Apply the event (currently only the show_text event)
        let text = try_unwrap(stack.pop(), COMPONENT_STACK_ERROR)?;
        try_unwrap(stack.last_mut(), COMPONENT_STACK_ERROR)?.hover_event =
            Some(Box::new(HoverEvent::show_text(text.into())));
    }
    // Every other event type
    else {
        let event_arg = try_unwrap(token_stack.pop(), TOKEN_STACK_ERROR)?;
        let event_name = try_unwrap(token_stack.pop(), TOKEN_STACK_ERROR)?;
        let event_type = try_unwrap(token_stack.pop(), TOKEN_STACK_ERROR)?;
        let component = try_unwrap(stack.last_mut(), COMPONENT_STACK_ERROR)?;

        // Hover events
        if event_type == "hover" {
            match event_name.as_ref() {
                "show_item" =>
                    component.hover_event = Some(Box::new(HoverEvent::show_item_json(&event_arg))),

                "show_entity" => {
                    // Make sure it parses the JSON correctly
                    match HoverEvent::show_entity_json(&event_arg) {
                        Some(event) => component.hover_event = Some(Box::new(event)),
                        None =>
                            return Err(CfmtError(
                                "Invalid argument for hover event \"show_entity\"".to_owned(),
                            )),
                    }
                }

                // Checks beforehand make this unreachable
                _ => {}
            }
        }
        // Click events
        else {
            match event_name.as_ref() {
                "open_url" =>
                    component.click_event = Some(Box::new(ClickEvent::open_url(event_arg))),
                "run_command" =>
                    component.click_event = Some(Box::new(ClickEvent::run_command(event_arg))),
                "suggest_command" =>
                    component.click_event = Some(Box::new(ClickEvent::suggest_command(event_arg))),
                "change_page" => {
                    // Parse the page index
                    match event_arg.parse::<u32>() {
                        Ok(index) =>
                            component.click_event = Some(Box::new(ClickEvent::change_page(index))),
                        Err(_) =>
                            return Err(CfmtError(
                                "Invalid page index for click event \"change_page\"".to_owned(),
                            )),
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}

// Add the child component but check to see if the previous child is just white space and can be
// combined with the given child
fn add_child(parent: &mut TextComponent, mut child: TextComponent) -> Result<(), CfmtError> {
    match parent.extra.as_mut() {
        // Children present, time to check
        Some(children) => {
            // The last child should always be a text component, but we have to match it
            match children.last_mut() {
                // Unpack text component
                Some(Component::Text(prev_child)) => {
                    // Is the previous child just whitespace without any formatting that could be displayed
                    if prev_child.text.trim().is_empty()
                        && !prev_child.underline.unwrap_or(false)
                        && !prev_child.strikethrough.unwrap_or(false)
                    {
                        // Combine the text of the two components
                        let mut text = prev_child.text.to_owned();
                        text.push_str(&child.text);
                        child.text = text;

                        // The whitespace child component is no longer needed
                        children.pop();
                    }
                }

                // Unreachable
                _ =>
                    return Err(CfmtError(
                        "Internal parser error: invalid state reached while appending a child \
                         component."
                            .to_owned(),
                    )),
            }
        }

        // No children present, nothing to check
        None => {}
    }

    parent.add_child(Component::Text(child));
    Ok(())
}

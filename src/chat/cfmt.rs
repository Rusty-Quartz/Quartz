use crate::chat::component::*;

#[macro_export]
macro_rules! component {
    ($cfmt:expr, $($arg:expr)*) => {
        crate::chat::cfmt::from_cfmt(&format!($cfmt, $($arg)*))
    };
    ($cfmt:expr) => {
        crate::chat::cfmt::from_cfmt($cfmt)
    };
}

pub fn from_cfmt(cfmt: &str) -> Result<Component, String> {
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
        ($format:expr, $len:expr $(, $arg:tt)*) => {{
            let idx = *index_stack.last().unwrap();

            let mut right = idx + $len.min(35);
            if right > cfmt.len() {
                right = cfmt.len();
            }

            return Err(format!($format, &cfmt[idx..right], $($arg)*));
        }};
        ($msg:expr) => {{
            return Err(String::from($msg));
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
        () => {
            index_stack.pop();
        };
    }

    macro_rules! current_token {
        () => {
            token_stack.last_mut().unwrap()
        };
    }

    macro_rules! finish_event {
        () => {
            // Handle stack operations and retrieve the component argument
            if component_type == EVENT {
                // Add any remaining child
                if has_children.pop().unwrap() {
                    let child = stack.pop().unwrap();
                    add_child(stack.last_mut().unwrap(), child);
                }

                // Apply the event (currently only the show_text event)
                let text = stack.pop().unwrap();
                stack.last_mut().unwrap().hover_event = Some(Box::new(HoverEvent::show_text(text)));

                // Reset the component type
                component_type = NORMAL;
            }
            // Every other event type
            else {
                let event_arg = token_stack.pop().unwrap();
                let event_name = token_stack.pop().unwrap();
                let event_type = token_stack.pop().unwrap();
                let component = stack.last_mut().unwrap();

                // Hover events
                if event_type == "hover" {
                    match event_name.as_ref() {
                        "show_item" => {
                            // Make sure it parses the JSON correctly
                            match HoverEvent::show_item(&event_arg) {
                                Some(event) => component.hover_event = Some(Box::new(event)),
                                None => {
                                    error!(
                                        "Invalid argument for hover event \"show_item\": \"{}\"",
                                        index - index_stack.last().unwrap()
                                    );
                                }
                            }
                        },

                        "show_entity" => {
                            // Make sure it parses the JSON correctly
                            match HoverEvent::show_entity(&event_arg) {
                                Some(event) => component.hover_event = Some(Box::new(event)),
                                None => {
                                    error!(
                                        "Invalid argument for hover event \"show_entity\": \"{}\"",
                                        index - index_stack.last().unwrap()
                                    );
                                }
                            }
                        },

                        // Checks beforehand make this unreachable
                        _ => {}
                    }
                }
                // Click events
                else {
                    match event_name.as_ref() {
                        "open_url" => component.click_event = Some(Box::new(ClickEvent::open_url(event_arg))),
                        "run_command" => component.click_event = Some(Box::new(ClickEvent::run_command(event_arg))),
                        "suggest_command" => component.click_event = Some(Box::new(ClickEvent::suggest_command(event_arg))),
                        "change_page" => {
                            // Parse the page index
                            match event_arg.parse::<u32>() {
                                Ok(index) => component.click_event = Some(Box::new(ClickEvent::change_page(index))),
                                Err(_) => {
                                    error!(
                                        "Invalid page index for click event \"change_page\": \"{}\"",
                                        index - index_stack.last().unwrap()
                                    );
                                }
                            }
                        },
                        _ => {}
                    }
                }
            }
        };
    }

    // Take the top component off the stack and add it to the next component on the stack
    macro_rules! collapse {
        () => {{
            let child = stack.pop().unwrap();
            if !child.is_empty() {
                add_child(stack.last_mut().unwrap(), child);
            }
        }};
    }

    for ch in cfmt.chars() {
        match state {
            ADD_TEXT => {
                match ch {
                    '\\' => state = FORCE_ADD,

                    '&' => {
                        finish_component(&mut stack, &mut has_children);

                        state = COLOR_START;
                        token_stack.push(String::with_capacity(8));

                        // Mark the start of the sequence
                        mark!();
                    },

                    '{' => {
                        // Manage depth
                        curly_bracket_depth += 1;

                        // Keep the current component on the stack for reference when determining the format of the
                        // component after this block
                        let next;
                        if *has_children.last().unwrap() {
                            next = TextComponent::copy_formatting(String::new(), stack.last().unwrap());
                        } else {
                            next = TextComponent::new(String::new(), None);
                        }
                        stack.push(next);

                        // Allows the appended component to have children
                        has_children.push(false);

                        // Used to identify unpaired curly braces
                        mark!();
                    },

                    '}' => {
                        // Somone has a random close curly bracket lying around
                        if curly_bracket_depth == 0 {
                            mark!();
                            error!("Unpaired curly bracket: \"{}...\"", 10);
                        }

                        curly_bracket_depth -= 1;

                        // If children were appended in the block, append the current one before the block is closed
                        if has_children.pop().unwrap() {
                            collapse!();
                        }

                        // The component representing the current block
                        let block = stack.pop().unwrap();
                        // The component to attach the block to
                        let last: &mut TextComponent;
                        // The next component to push on the stack
                        let next: TextComponent;

                        // The outer component to attach this block to has children
                        if *has_children.last().unwrap() {
                            // Grab the component we left behind earlier to get the format for the next component
                            let reference = stack.pop().unwrap();

                            // Copy formatting from the reference
                            next = TextComponent::copy_formatting(String::new(), &reference);

                            // Append the component we left behind
                            last = stack.last_mut().unwrap();
                            add_child(last, reference);
                        }
                        // The outer component does not have any children, we are the first child
                        else {
                            // Formatting inherited from the outer block
                            next = TextComponent::new(String::new(), None);

                            last = stack.last_mut().unwrap();

                            // Now the outer component will have children
                            *has_children.last_mut().unwrap() = true;
                        }

                        // Append this block
                        add_child(last, block);
                        stack.push(next);
                    },

                    '$' => {
                        // Don't allow nesting of events
                        if component_type == EVENT {
                            error!("Events cannot be nested within components attached to events.");
                        }

                        finish_component(&mut stack, &mut has_children);

                        state = EVENT_START;

                        // Mark the start of the event sequence
                        mark!();
                    },

                    ')' => {
                        // This only happens for event components
                        if component_type == EVENT {
                            finish_event!();

                            state = ADD_TEXT;

                            unmark!();
                        }
                    },

                    _ => stack.last_mut().unwrap().text.push(ch)
                }
            },

            FORCE_ADD => {
                stack.last_mut().unwrap().text.push(ch);
                state = ADD_TEXT;
            },

            COLOR_START => {
                if ch == '(' {
                    state = COLOR_BUILD_FIRST;
                    current_token!().push('\"'); // For serde compatability

                    // Mark the start of the first color/format
                    mark!(1);
                } else {
                    error!("Expected open parenthesis after '&': \"{}...\"", 10);
                }
            },

            COLOR_BUILD_FIRST | COLOR_BUILD_EXTRA => {
                match ch {
                    ',' | ')' => {
                        let mut token = token_stack.pop().unwrap();

                        // Check for dangling comma
                        if ch == ')' && token.len() == 1 {
                            // Remove mark for the current color/format
                            unmark!();

                            error!("Dangling comma at the end of formatting sequence: \"{}\"", index - index_stack.last().unwrap() + 1);
                        }

                        token.push('\"');
                        let mut component = stack.last_mut().unwrap();

                        // Match the current item to a color
                        if state == COLOR_BUILD_FIRST {
                            if let Ok(color) = serde_json::from_str::<Color>(&token) {
                                if !apply_format {
                                    error!(
                                        "Negation character not allowed in front of a color code: \"{}\"",
                                        index - index_stack.last().unwrap()
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
                                            "Excpected color or \"reset\" as first argument of color sequence: \"{}...\"",
                                            index - index_stack.last().unwrap()
                                        );
                                    }
                                    // The format or color name was incorrect
                                    else {
                                        error!(
                                            "Invalid color or formatting code: \"{}\"",
                                            index - index_stack.last().unwrap()
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
                    },

                    '!' => {
                        // Valid syntax, ex: !bold
                        if current_token!().len() == 1 {
                            apply_format = false;
                        }
                        // One exclamation in the middle of the word, probably a typo
                        else if apply_format {
                            error!(
                                "Expected negation character ('!') to be at the beginning of a formatting code: \"{}...\"",
                                index - index_stack.last().unwrap() + 3
                            );
                        }
                        // Just pass this mess down to the format parser for the error
                        else {
                            current_token!().push(ch);
                        }
                    },

                    _ => current_token!().push(ch)
                }
            },

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
            },

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
                            index - index_stack.last().unwrap()
                        );
                    }
                }
                // If it's not the delimeter, append the character to the token
                else {
                    current_token!().push(ch);
                }
            },

            EVENT_BUILD_NAME => {
                // Separate the event data from its argument witha comma
                if ch == ',' {
                    let event_type: &str = token_stack[token_stack.len() - 2].as_ref();
                    let event_name: &str = token_stack.last().unwrap();
                    
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
                                },

                                // Both of these are JSON, so a string
                                "show_item" | "show_entity" => state = EVENT_BUILD_ARG,

                                // Invalid event name
                                _ => {
                                    error!(
                                        "Invalid event name for hover type: \"{}\"",
                                        index - index_stack.last().unwrap()
                                    );
                                }
                            }
                        },

                        "click" => {
                            // Valid click events
                            if event_name == "open_url" || event_name == "run_command" ||
                                    event_name == "suggest_command" || event_name == "change_page" {
                                state = EVENT_BUILD_ARG;
                            }
                            // Invalid click event
                            else {
                                error!(
                                    "Invalid event name for click type: \"{}\"",
                                    index - index_stack.last().unwrap()
                                );
                            }
                        },

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
            },

            EVENT_BUILD_ARG => {
                // Event sequence ends with a close parentehsis
                if ch == ')' {
                    finish_event!();

                    state = ADD_TEXT;

                    unmark!();
                } else {
                    current_token!().push(ch);
                }
            },

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

            return Ok(Component::Text(stack.pop().unwrap()))
        },

        FORCE_ADD => {
            error!("Expected another character after the escape character at the end of the input string.");
        },

        COLOR_START | COLOR_BUILD_FIRST | COLOR_BUILD_EXTRA => {
            error!("Incomplete color sequence at the end of the input string.");
        },

        EVENT_START | EVENT_BUILD_NAME | EVENT_BUILD_TYPE | EVENT_BUILD_ARG => {
            error!("Incomplete event sequence at the end of the input string.");
        }
        
        _ => return Ok(Component::Text(stack.pop().unwrap()))
    }
}

fn finish_component(stack: &mut Vec<TextComponent>, has_children: &mut Vec<bool>) {
    if !stack.last().unwrap().is_empty() {
        // Some children are already present
        if *has_children.last().unwrap() {
            let child = stack.pop().unwrap();

            // Manage inheritance here to prevent the JSON depth from getting insane
            let next = TextComponent::copy_formatting(String::new(), &child);

            add_child(stack.last_mut().unwrap(), child);
            stack.push(next);
        }
        // Add the first child
        else {
            stack.push(TextComponent::new(String::new(), None));
            *has_children.last_mut().unwrap() = true;
        }
    }
}

// Add the child component but check to see if the previous child is just white space and can be
// combined with the given child
fn add_child(parent: &mut TextComponent, mut child: TextComponent) {
    match parent.extra.as_mut() {
        // Children present, time to check
        Some(children) => {
            // The last child should always be a text component, but we have to match it
            match children.last_mut().unwrap() {
                // Unpack text component
                Component::Text(prev_child) => {
                    // Is the previous child just whitespace without any formatting that could be displayed
                    if prev_child.text.trim().is_empty() &&
                            !(prev_child.underline.is_some() && prev_child.underline.unwrap()) &&
                            !(prev_child.strikethrough.is_some() && prev_child.strikethrough.unwrap()) {
                        // Combine the text of the two components
                        let mut text = String::from(&prev_child.text);
                        text.push_str(&child.text);
                        child.text = text;

                        // The whitespace child component is no longer needed
                        children.pop();
                    }
                },

                // Unreachable
                _ => {}
            }
        },

        // No children present, nothing to check
        None => {}
    }

    parent.add_child(Component::Text(child));
}
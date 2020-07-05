use crate::command::executor::{CommandContext, ExecutableCommand};

use lazy_static::lazy_static;
use regex::Regex;

/// Iterates over the individual arguments in a command accounting for quoted arguments.
pub struct ArgumentTraverser<'cmd> {
    command: &'cmd str,
    anchor: usize,
    index: usize,
    paused: bool
}

impl<'cmd> ArgumentTraverser<'cmd> {
    /// Creates a traverser over the given command, stripping off the initial '/' if it exists.
    pub fn new(command: &'cmd str) -> Self {
        ArgumentTraverser {
            command: if command.starts_with('/') { &command[1..] } else { command },
            anchor: 0,
            index: 0,
            paused: false
        }
    }

    /// Pauses this traverser, meaning the next call to `next` will return the same value as the
    /// previous call while also unpausing the traverser.
    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Returns the remaining portion of the string being traversed, including the argument which
    /// was last read.
    pub fn remaining(&mut self) -> &'cmd str {
        self.index = self.command.len();
        &self.command[self.anchor..]
    }

    /// Returns the remaining portion of the string being traversed from the current anchor
    /// position to the end of the string.
    pub fn remaining_truncated(&self, truncate_to: usize) -> &'cmd str {
        if self.command.len() - self.anchor > truncate_to {
            &self.command[self.anchor..self.anchor + truncate_to]
        } else {
            &self.command[self.anchor..]
        }
    }

    /// Returns whether or not this traverser has more arguments. If this function returns true, then
    /// `next` will not return `None`.
    pub fn has_next(&self) -> bool {
        self.index < self.command.len()
    }

    /// Returns the next argument in the string being traversed, or `None` if no arguments remain.
    pub fn next(&mut self) -> Option<&'cmd str> {
        if self.paused {
            self.paused = false;
            return Some(&self.command[self.anchor..self.index]);
        }

        if self.index >= self.command.len() {
            return None;
        }

        let bytes = self.command.as_bytes();

        // Skip leading spaces
        while self.index < self.command.len() && bytes[self.index] == ' ' as u8 {
            self.index += 1;
        }

        self.anchor = self.index;

        // Single or double quotes
        let mut quote_type: u8 = 0;
        // Whether we're in quotes and should ignore spaces
        let mut in_quotes = false;
        // Used for escaping quotes with the '\' character
        let mut ignore_quote = false;

        while self.index < self.command.len() && (in_quotes || bytes[self.index] != ' ' as u8) {
            // Manage strings
            if (bytes[self.index] == '\'' as u8 || bytes[self.index] == '\"' as u8) && !ignore_quote {
                if in_quotes {
                    if bytes[self.index] == quote_type {
                        in_quotes = false;
                    }
                } else {
                    quote_type = bytes[self.index];
                    in_quotes = true;
                }
            }

            // Unset the ignore quote variable
            if ignore_quote {
                ignore_quote = false;
            }

            // Set the ignore quote variable if the escape character is present
            if in_quotes && bytes[self.index] == '\\' as u8 {
                ignore_quote = true;
            }

            self.index += 1;
        }

        Some(&self.command[self.anchor..self.index])
    }
}

/// Acts both as a wrapper for argument values and argument type definitions.
#[derive(Clone)]
pub enum Argument<'cmd> {
    /// A signed integer argument, parsed as an `i64`.
    Integer(
        /// The argument value.
        i64
    ),
    /// A floating point argument, parsed as an `f64`.
    FloatingPoint(
        /// The argument value.
        f64
    ),
    /// A string argument in the form of a slice of the full command string.
    String(
        /// The argument value.
        &'cmd str
    ),
    /// An executable sub-command argument, which is a wrapper around a slice of the original command.
    Command(
        /// The argument value.
        ExecutableCommand<'cmd>
    )
}

impl<'cmd> Argument<'cmd> {
    /// Whether or not this argument type matches the given string. This does not guarantee a successful parse.
    pub fn matches(&self, argument: &str) -> bool {
        lazy_static! {
            static ref FLOAT: Regex = Regex::new(r"^(-+)?(\d+\.\d*)|(\d*\.\d+)$").unwrap();
            static ref INT: Regex = Regex::new(r"^(-+)?\d+$").unwrap();
        }

        match self {
            Argument::Integer(_value) => INT.is_match(argument),
            Argument::FloatingPoint(_value) => FLOAT.is_match(argument),
            Argument::String(_value) => true,
            Argument::Command(_) => false
        }
    }

    /// Whether or not the partial argument matches to this argument type.
    pub fn partial_match(&self, partial_argument: &str) -> bool {
        // TODO: Ensure this works correctly for more complex arguments
        self.matches(partial_argument)
    }

    /// Attempts to parse the given argument according to this argument's type. If the parse is successful, an argument
    /// of the same type is added to the context with the given name with the parsed value of the given string argument.
    pub fn apply<'ctx>(&self, context: &mut CommandContext<'ctx>, name: &'static str, argument: &'ctx str) -> Result<(), String> {
        match self {
            Argument::Integer(_value) => {
                let parsed = argument.parse::<i64>();
                if parsed.is_err() {Err("Invalid Integer".to_owned())}
                else {
                    context.arguments.insert(name, Argument::Integer(parsed.unwrap()));
                    Ok(())
                }
            },

            Argument::FloatingPoint(_value) => {
                let parsed = argument.parse::<f64>();
                if parsed.is_err() {Err("Invalid Integer".to_owned())}
                else {
                    context.arguments.insert(name, Argument::FloatingPoint(parsed.unwrap()));
                    Ok(())
                }
            },

            Argument::String(_value) => {
                context.arguments.insert(name, Argument::String(argument));
                Ok(())
            },
            
            Argument::Command(_) => Ok(())
        }
    }
}
use crate::command::executor::CommandContext;

use lazy_static::lazy_static;
use regex::Regex;

// Iterates over the individual arguments in a command accounting for ignored spaces
pub struct ArgumentTraverser<'cmd> {
    command: &'cmd str,
    anchor: usize,
    index: usize
}

impl<'cmd> ArgumentTraverser<'cmd> {
    pub fn new(command: &'cmd str) -> Self {
        ArgumentTraverser {
            command,
            anchor: 0,
            index: 0
        }
    }

    pub fn remaining(&mut self) -> String {
        self.index = self.command.len();
        self.command[self.anchor..].to_owned()
    }

    // Returns the remaining string portion from the current anchor position to the end of the string
    pub fn remaining_truncated(&self, truncate_to: usize) -> &'cmd str {
        if self.command.len() - self.anchor > truncate_to {
            &self.command[self.anchor..self.anchor + truncate_to]
        } else {
            &self.command[self.anchor..]
        }
    }
}

impl<'cmd> Iterator for ArgumentTraverser<'cmd> {
    type Item = &'cmd str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.command.len() {
            return None;
        }

        self.anchor = self.index;

        // Single or double quotes
        let mut quote_type: u8 = 0;
        // Whether we're in quotes and should ignore spaces
        let mut in_quotes = false;
        // Used for escaping quotes with the '\' character
        let mut ignore_quote = false;

        let bytes = self.command.as_bytes();
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

        let result = &self.command[self.anchor..self.index];
        self.index += 1; // Skip the space between the arguments
        Some(result)
    }
}

// Acts both as a wrapper for argument values and argument type definition
#[derive(Clone)]
pub enum Argument {
    Remaining(String),
    Literal(&'static str),
    Integer(i64),
    FloatingPoint(f64),
    String(String)
}

impl Argument {
    // Whether or not this argument type matches the given string (does not guarantee a successful parse)
    pub fn matches(&self, argument: &str) -> bool {
        lazy_static! {
            static ref FLOAT: Regex = Regex::new(r"^(-+)?(\d+\.\d*)|(\d*\.\d+)$").unwrap();
            static ref INT: Regex = Regex::new(r"^(-+)?\d+$").unwrap();
        }

        match self {
            Argument::Remaining(_value) => true,
            Argument::Literal(literal) => literal.eq_ignore_ascii_case(argument),
            Argument::Integer(_value) => INT.is_match(argument),
            Argument::FloatingPoint(_value) => FLOAT.is_match(argument),
            Argument::String(_value) => true
        }
    }

    // Attempts to parse the given argument according to this arguments type. If the parse is successful, an argument
    // of the same type is added to the context with the parsed value of the given string argument with the given name.
    pub fn apply(&self, context: &mut CommandContext, name: &'static str, argument: &str) -> Result<(), String> {
        match self {
            Argument::Remaining(_value) => {
                // Notify the arg loop that it should break
                context.arguments.insert(name.to_owned(), Argument::Remaining(context.raw_args.remaining()));
                Ok(())
            },
            Argument::Literal(_value) => {
                context.arguments.insert(name.to_owned(), self.clone());
                Ok(())
            },
            Argument::Integer(_value) => {
                let parsed = argument.parse::<i64>();
                if parsed.is_err() {Err("Invalid Integer".to_owned())}
                else {
                    context.arguments.insert(name.to_owned(), Argument::Integer(parsed.unwrap()));
                    Ok(())
                }
            },
            Argument::FloatingPoint(_value) => {
                let parsed = argument.parse::<f64>();
                if parsed.is_err() {Err("Invalid Integer".to_owned())}
                else {
                    context.arguments.insert(name.to_owned(), Argument::FloatingPoint(parsed.unwrap()));
                    Ok(())
                }
            },
            Argument::String(_value) => {
                context.arguments.insert(name.to_owned(), Argument::String(argument.to_owned()));
                Ok(())
            }
        }
    }
}
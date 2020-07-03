use std::collections::HashMap;

use crate::server::QuartzServer;

use crate::command::arg::*;
use crate::command::CommandSender;

use chat::{
    Component,
    color::PredefinedColor
};

// Contains a map of commands which can be executed
pub struct CommandExecutor<'sv> {
    commands: HashMap<String, CommandNode<'sv>>,
    descriptions: HashMap<String, String>
}

impl<'sv> CommandExecutor<'sv> {
    pub fn new() -> Self {
        CommandExecutor {
            commands: HashMap::new(),
            descriptions: HashMap::new()
        }
    }

    /// Registers the given command node with its defined syntax. If the node is not a literal,
    /// then this function currently just ignores it and does nothing.
    pub fn register(&mut self, node: CommandNode<'sv>, description: &str) {
        match node.argument {
            Argument::Literal(name) => {
                self.commands.insert(name.to_owned(), node);
                self.descriptions.insert(name.to_owned(), description.to_owned());
            },
            // Perhaps consider handling this error
            _ => {}
        }
    }

    /// Attempts to dispatch the given command, first attempting to parse it according to the registered command
    /// syntax trees, and then creating a context in which it can be executed. If an error occurs at some point,
    /// then the sender is notified.
    pub fn dispatch(&self, command: &str, server: &QuartzServer, sender: CommandSender) {
        let mut context = CommandContext {
            server,
            executor: &self,
            sender,
            arguments: HashMap::new(),
            raw_args: ArgumentTraverser::new(command)
        };

        // Find the name of the command
        let root_name = match context.raw_args.next() {
            Some(arg) => arg,
            // Empty command, just exit silently
            None => return
        };

        // The root node of the command
        let root = match self.commands.get(root_name) {
            Some(root) => root,
            None => {
                context.sender.send_message(
                    Component::colored(format!("No command found named \"{}\"", root_name), PredefinedColor::Red)
                );
                return;
            }
        };

        // The current node we're at in the command tree
        let mut node = root;

        // Iterate over the avilable argument and trace a path on the command tree
        'arg_loop: while let Some(arg) = context.raw_args.next() {
            // The current node has no children but there are still arguments remaining, so notify
            // the sender that some arguments are getting ignored
            if node.children.is_empty() {
                // Extra arguments
                context.sender.send_message(
                    Component::colored(
                        format!("Ignoring the following arguments: \"{}\"", context.raw_args.remaining_truncated(35)),
                        PredefinedColor::Red
                    )
                );

                node.execute(context);
                return;
            }
            // Find the next node in the tree
            else {
                // Find a child that matches the given argument and attempt to apply it
                for child in node.children.iter().filter(|child| child.argument.matches(arg)) {
                    match child.argument.apply(&mut context, child.name, arg) {
                        Ok(()) => {
                            node = child;
                            // Only use the first child that matches
                            continue 'arg_loop;
                        },
                        Err(e) => {
                            context.sender.send_message(
                                Component::colored(format!("Invalid value for argument \"{}\": {}", child.name, e), PredefinedColor::Red)
                            );
                            return;
                        }
                    }
                }

                // We couldn't match the given argument to a node
                if node.children.len() == 1 {
                    context.sender.send_message(
                        Component::colored(
                            format!("Invalid value for argument \"{}\": \"{}\"", node.children[0].name, arg),
                            PredefinedColor::Red
                        )
                    );
                } else {
                    Self::expect_args(node, &context);
                }

                return;
            }
        }

        // Look for and load in default argument values
        'default_loop: while !node.children.is_empty() {
            for child in node.children.iter() {
                if child.default {
                    node = child;
                    context.arguments.insert(child.name.to_owned(), child.argument.clone());
                    continue 'default_loop;
                }
            }

            // No defaults were found
            break 'default_loop;
        }

        // Handle the expectation for more arguments if needed
        match &node.executor {
            Some(executor) => executor(context),
            None => match node.children.len() {
                0 => {},
                1 => {
                    context.sender.send_message(
                        Component::colored(format!("Expected value for argument \"{}\"", node.children[0].name), PredefinedColor::Red)
                    );
                },
                _ => Self::expect_args(node, &context)
            }
        }
    }

    // We were expecting more arguments, so notify the sender what was expected
    fn expect_args(node: &CommandNode, context: &CommandContext) {
        let mut message = "Expected one of the following arguments: ".to_owned();
        message.push_str(node.children[0].name);
        for child in node.children.iter().skip(1) {
            message.push_str(", ");
            message.push_str(child.name);
        }
        context.sender.send_message(Component::colored(message, PredefinedColor::Red));
    }
    
    pub fn command_names(&self) -> Vec<&String> {
        self.commands.keys().collect()
    }

    pub fn command_description(&self, command: &str) -> Option<&String> {
        self.descriptions.get(command)
    }
}

/// The context in which a command is executed. This has no use outside the lifecycle
/// of a command.
pub struct CommandContext<'ctx> {
    pub server: &'ctx QuartzServer<'ctx>,
    pub executor: &'ctx CommandExecutor<'ctx>,
    pub sender: CommandSender,
    pub arguments: HashMap<String, Argument>,
    pub raw_args: ArgumentTraverser<'ctx>
}

// Shortcut functions for getting argument values
impl<'ctx> CommandContext<'ctx> {
    pub fn get_integer(&self, key: &str) -> Option<i64> {
        match self.arguments.get(key) {
            Some(Argument::Integer(value)) => Some(*value),
            _ => None
        }
    }

    pub fn get_float(&self, key: &str) -> Option<f64> {
        match self.arguments.get(key) {
            Some(Argument::FloatingPoint(value)) => Some(*value),
            _ => None
        }
    }

    pub fn get_string(&self, key: &str) -> Option<String> {
        match self.arguments.get(key) {
            Some(Argument::String(value)) => Some(value.to_owned()),
            _ => None
        }
    }
}

// The basic structural unit of a command syntax tree
pub struct CommandNode<'sv> {
    name: &'static str,
    argument: Argument,
    executor: Option<Box<dyn Fn(CommandContext) + 'sv>>,
    children: Vec<CommandNode<'sv>>,
    default: bool
}

impl<'sv> CommandNode<'sv> {
    #[inline]
    fn new(name: &'static str, arg: Argument, default: bool) -> CommandNode<'sv> {
        CommandNode {
            name,
            argument: arg,
            executor: None,
            children: Vec::new(),
            default
        }
    }

    // Adds a child
    pub fn then(mut self, child: CommandNode<'sv>) -> CommandNode<'sv> {
        self.children.push(child);
        self
    }

    // Adds an executor
    pub fn executes(mut self, executor: impl Fn(CommandContext) + 'sv) -> Self {
        self.executor = Some(Box::new(executor));
        self
    }

    // Attempts to execute the node with the given context. Returns whether or not an executor was called
    pub fn execute(&self, context: CommandContext) -> bool {
        match &self.executor {
            Some(executor) => {
                executor(context);
                true
            },
            None => false
        }
    }
}

// Breaks the argument iterator loop causing this node to be executed
#[inline]
pub fn remaining<'a>(name: &'static str) -> CommandNode<'a> {
    CommandNode::new(name, Argument::Remaining("".to_owned()), false)
}

// A command literal, or an exact string such as "foo"
#[inline]
pub fn literal(literal: &'static str) -> CommandNode {
    CommandNode::new(literal, Argument::Literal(literal), false)
}

// An integer value, signed or unsigned, parsed as an i64
#[inline]
pub fn integer(name: &'static str) -> CommandNode {
    CommandNode::new(name, Argument::Integer(0), false)
}

// An integer with a default value
#[inline]
pub fn integer_default(name: &'static str, default: i64) -> CommandNode {
    CommandNode::new(name, Argument::Integer(default), true)
}

// A floating point value parsed as an f64
#[inline]
pub fn float(name: &'static str) -> CommandNode {
    CommandNode::new(name, Argument::Integer(0), false)
}

// A floating point argument with a default value
#[inline]
pub fn float_default(name: &'static str, default: f64) -> CommandNode {
    CommandNode::new(name, Argument::FloatingPoint(default), true)
}

// A string argument, which is essentially just the raw argument
#[inline]
pub fn string(name: &'static str) -> CommandNode {
    CommandNode::new(name, Argument::String(String::new()), false)
}

// A string argument with a default value
#[inline]
pub fn string_default<'a>(name: &'static str, default: &str) -> CommandNode<'a> {
    CommandNode::new(name, Argument::String(default.to_owned()), true)
}

// After the given vec of args, look for the following node
pub fn after<'a>(mut args: Vec<CommandNode<'a>>, last: CommandNode<'a>) -> CommandNode<'a> {
    args.last_mut().unwrap().children.push(last);
    while args.len() > 1 {
        let node = args.pop().unwrap();
        args.last_mut().unwrap().children.push(node);
    }
    args.pop().unwrap()
}
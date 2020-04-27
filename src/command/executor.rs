use std::collections::HashMap;

use crate::QuartzServer;

use crate::command::arg::*;
use crate::command::CommandSender;

use crate::color;

pub struct CommandExecutor<'ex> {
    commands: HashMap<String, CommandNode<'ex>>
}

impl<'ex> CommandExecutor<'ex> {
    pub fn new() -> Self {
        CommandExecutor {
            commands: HashMap::new()
        }
    }

    pub fn register(&mut self, node: CommandNode<'ex>) {
        match node.argument {
            Argument::Literal(name) => {
                self.commands.insert(name.to_owned(), node);
            },
            // Perhaps consider handling this error
            _ => {}
        }
    }

    pub fn dispatch(&self, command: &str, server: &QuartzServer<'_>, sender: CommandSender) {
        let mut context = CommandContext {
            server,
            executor: &self,
            sender,
            arguments: HashMap::new()
        };

        let mut args = ArgumentTraverser::new(command);
        let root_name: &str;

        match args.next() {
            Some(arg) => root_name = arg,
            None => return
        }

        match self.commands.get(root_name) {
            Some(root) => {
                let mut node = root;

                'arg_loop: while let Some(arg) = args.next() {
                    if node.children.is_empty() {
                        // Extra arguments
                        context.sender.send_message(color!("Ignoring the following arguments: \"{}\"", Red, args.remaining_string(35)));

                        node.execute(&mut context);
                        return;
                    } else {
                        // Find a child that matches the given argument and attempt to apply it
                        for child in node.children.iter().filter(|child| child.argument.matches(arg)) {
                            match child.argument.apply(&mut context, child.name, arg) {
                                Ok(()) => node = child,
                                Err(e) => {
                                    context.sender.send_message(color!("Invalid value for argument \"{}\": {}", Red, child.name, e));
                                    return;
                                }
                            }

                            continue 'arg_loop;
                        }

                        // We couldn't match the given argument to a node
                        if node.children.len() == 1 {
                            context.sender.send_message(color!("Invalid value for argument \"{}\": \"{}\"", Red, node.children[0].name, arg));
                        } else {
                            Self::expect_args(node, &context);
                        }

                        return;
                    }
                }

                // Handle the expectation for more arguments if needed
                if !node.execute(&mut context) && !node.children.is_empty() {
                    if node.children.len() == 1 {
                        context.sender.send_message(color!("Expected value for argument \"{}\"", Red, node.children[0].name));
                    } else {
                        Self::expect_args(node, &context);
                    }
                }
            },
            None => context.sender.send_message(color!("No command found named \"{}\"", Red, root_name))
        }
    }

    fn expect_args(node: &CommandNode, context: &CommandContext) {
        let mut message = String::from("Expected one of the following arguments: ");
        message.push_str(node.children[0].name);
        for child in node.children.iter().skip(1) {
            message.push_str(", ");
            message.push_str(child.name);
        }
        context.sender.send_message(color!(message, Red));
    }
}

pub struct CommandContext<'ctx> {
    pub server: &'ctx QuartzServer<'ctx>,
    pub executor: &'ctx CommandExecutor<'ctx>,
    pub sender: CommandSender,
    pub arguments: HashMap<String, Argument>
}

pub struct CommandNode<'ex> {
    name: &'static str,
    argument: Argument,
    executor: Option<Box<dyn Fn(&mut CommandContext) + 'ex>>,
    children: Vec<CommandNode<'ex>>
}

impl<'ex> CommandNode<'ex> {
    #[inline]
    fn new(name: &'static str, arg: Argument) -> CommandNode<'ex> {
        CommandNode {
            name,
            argument: arg,
            executor: None,
            children: Vec::new()
        }
    }

    pub fn then(mut self, child: CommandNode<'ex>) -> CommandNode<'ex> {
        self.children.push(child);
        self
    }

    pub fn executes(mut self, executor: impl Fn(&mut CommandContext) + 'ex) -> Self {
        self.executor = Some(Box::new(executor));
        self
    }

    pub fn execute(&self, context: &mut CommandContext) -> bool {
        match &self.executor {
            Some(executor) => {
                executor(context);
                true
            },
            None => false
        }
    }
}

pub fn executor<'a>(executor: impl Fn(&mut CommandContext) + 'a) -> CommandNode<'a> {
    CommandNode::new("", Argument::Any).executes(executor)
}

pub fn argchain(args: Vec<CommandNode>) -> CommandNode {
    while args.len() > 1 {
        let node = args.pop().unwrap();
        args.last_mut().unwrap().children.push(node);
    }
    args.pop().unwrap()
}

#[inline]
pub fn literal(literal: &'static str) -> CommandNode {
    CommandNode::new(literal, Argument::Literal(literal))
}

#[inline]
pub fn integer(name: &'static str) -> CommandNode {
    CommandNode::new(name, Argument::Integer(0))
}
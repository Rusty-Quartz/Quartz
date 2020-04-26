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

    pub fn register(&mut self, command: &str, node: CommandNode<'ex>) {
        self.commands.insert(command.to_owned(), node);
    }

    pub fn dispatch(&self, command: &str, server: &QuartzServer<'_>, sender: CommandSender) {
        let mut context = CommandContext {
            server,
            executor: &self,
            sender
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
                for arg in args {
                    // TODO: traverse arguments
                }

                if !node.execute(&mut context) {
                    // TODO: handle the fact that more args were expected
                }
            },
            None => context.sender.send_message(color!("No command found named \"{}\"", Red, root_name))
        }
    }
}

pub struct CommandContext<'ctx> {
    pub server: &'ctx QuartzServer<'ctx>,
    pub executor: &'ctx CommandExecutor<'ctx>,
    pub sender: CommandSender
}

pub struct CommandNode<'ex> {
    argument: Argument,
    executor: Option<Box<dyn Fn(&mut CommandContext) + 'ex>>,
    children: Vec<CommandNode<'ex>>
}

impl<'ex> CommandNode<'ex> {
    #[inline]
    fn new(arg: Argument) -> CommandNode<'ex> {
        CommandNode {
            argument: arg,
            executor: None,
            children: Vec::new()
        }
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
    CommandNode::new(Argument::Any).executes(executor)
}

#[inline]
pub fn literal(literal: &'static str) -> CommandNode {
    CommandNode::new(Argument::Literal(literal))
}
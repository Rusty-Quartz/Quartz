use std::collections::HashMap;

use crate::server::QuartzServer;

use crate::command::arg::*;
use crate::command::CommandSender;

use chat::{
    Component,
    color::PredefinedColor
};

// Contains a map of commands which can be executed
pub struct CommandExecutor {
    commands: HashMap<&'static str, CommandNode>,
    descriptions: HashMap<&'static str, String>
}

impl CommandExecutor {
    pub fn new() -> Self {
        CommandExecutor {
            commands: HashMap::new(),
            descriptions: HashMap::new()
        }
    }

    /// Registers the given command node with its defined syntax. If the node is not a literal,
    /// then this function currently just ignores it and does nothing.
    pub fn register(&mut self, node: CommandNode, description: &str) {
        match &node {
            CommandNode::Literal {base, ..} => {
                let name = base.name;
                self.commands.insert(name, node);
                self.descriptions.insert(name, description.to_owned());
            },
            // Perhaps consider handling this error
            _ => {}
        }
    }

    /// Attempts to dispatch the given command, first attempting to parse it according to the registered command
    /// syntax trees, and then creating a context in which it can be executed. If an error occurs at some point,
    /// then the sender is notified.
    pub fn dispatch(&self, command: &str, server: &QuartzServer, sender: CommandSender) {
        let mut context = CommandContext::new(server, self, sender);
        let mut raw_args = ArgumentTraverser::new(command);

        // Find the name of the command
        let root_name = match raw_args.next() {
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

        // Create the parser state with the arguments and root node
        let mut state = ParserState::new(raw_args, root);

        // Iterate over the avilable argument and trace a path on the command tree
        'arg_loop: while let Some(arg) = state.next_argument() {
            // Find the next node in the tree
            if state.current_node.has_children() {
                let children = match state.current_node.children() {
                    Some(children) => children,
                    None => return
                };

                // Find a child that matches the given argument and attempt to apply it
                for child in children.iter().filter(|child| child.matches(arg)) {
                    match child.apply(&mut state, &mut context, arg) {
                        Ok(()) => {
                            // Only use the first child that matches
                            continue 'arg_loop;
                        },
                        Err(e) => {
                            context.sender.send_message(e);
                            return;
                        }
                    }
                }

                // We couldn't match the given argument to a node
                if children.len() == 1 {
                    context.sender.send_message(
                        Component::colored(
                            format!("Invalid value for argument \"{}\": \"{}\"", children[0].name(), arg),
                            PredefinedColor::Red
                        )
                    );
                } else {
                    Self::expect_args(children, &context);
                }

                return;
            }
            // The current node has no children but there are still arguments remaining, so notify
            // the sender that some arguments are getting ignored
            else {
                // Extra arguments
                context.sender.send_message(
                    Component::colored(
                        format!("Ignoring the following arguments: \"{}\"", state.raw_args.remaining_truncated(35)),
                        PredefinedColor::Red
                    )
                );

                state.current_node.execute(context);
                return;
            }
        }

        // Look for and load in default argument values
        'default_loop: while state.current_node.has_children() {
            for child in state.current_node.children().into_iter().flatten() {
                match child {
                    CommandNode::Argument {base, argument, default, ..} => {
                        if *default {
                            state.current_node = child;
                            context.arguments.insert(base.name, argument.clone());
                            continue 'default_loop;
                        }
                    },
                    _ => continue
                }
            }

            // No defaults were found
            break 'default_loop;
        }

        // Handle the expectation for more arguments if needed
        match &state.current_node.executor() {
            Some(executor) => executor(context),
            None => match state.current_node.children() {
                Some(children) => match children.len() {
                    0 => {},
                    1 => {
                        context.sender.send_message(
                            Component::colored(
                                format!("Expected value for argument \"{}\"", children[0].name()),
                                PredefinedColor::Red
                            )
                        );
                    },
                    _ => Self::expect_args(children, &context)
                },
                None => {}
            }
        }
    }

    pub fn get_suggestions(&self, command: &str, server: &QuartzServer, sender: CommandSender) -> Vec<String> {
        let mut context = CommandContext::new(server, self, sender);
        let mut raw_args = ArgumentTraverser::new(command);

        // Find the name of the command
        let root_name = match raw_args.next() {
            Some(arg) => arg,
            // Empty command, just exit silently
            None => return self.commands.keys().map(|cmd| (*cmd).to_owned()).collect()
        };

        if !raw_args.has_next() {
            return self.commands.keys().filter(|cmd| cmd.starts_with(root_name)).map(|cmd| (*cmd).to_owned()).collect();
        }

        let root = match self.commands.get(root_name) {
            Some(root) => root,
            None => return Vec::new()
        };

        // Create the parser state with the arguments and root node
        let mut state = ParserState::new(raw_args, root);

        // Iterate over the avilable argument and trace a path on the command tree
        'arg_loop: while let Some(arg) = state.next_argument() {
            // Find the next node in the tree
            if state.current_node.has_children() {
                // Unwrap is safe because of the check above
                let children = match state.current_node.children() {
                    Some(children) => children,
                    None => return Vec::new()
                };

                if state.raw_args.has_next() {
                    // Find a child that matches the given argument and attempt to apply it
                    for child in children.iter().filter(|child| child.matches(arg)) {
                        match child.apply(&mut state, &mut context, arg) {
                            Ok(_) => {
                                // Only use the first child that matches
                                continue 'arg_loop;
                            },
                            Err(_) => return Vec::new()
                        }
                    }
                } else {
                    // Generate suggestions
                    let mut suggestions: Vec<String> = Vec::new();
                    for child in children.iter().filter(|child| child.partial_match(arg)) {
                        child.add_suggestions(&state, &context, arg, &mut suggestions);
                    }
                    return suggestions;
                }

                return Vec::new();
            }
            // The current node has no children so there is nothing left to suggest
            else {
                return Vec::new();
            }
        }

        // If we got redirected to the root, then recursively give command suggestions
        match state.current_node {
            CommandNode::Redirection {base, ..} => match context.get_command(base.name) {
                Some(command) => self.get_suggestions(command.0, server, context.sender),
                None => Vec::new()
            },
            _ => Vec::new()
        }
    }

    // We were expecting more arguments, so notify the sender what was expected
    fn expect_args(children: &Vec<CommandNode>, context: &CommandContext) {
        let mut message = "Expected one of the following arguments: ".to_owned();
        message.push_str(&children.iter().map(|child| child.name()).collect::<Vec<&'static str>>().join(", "));
        context.sender.send_message(Component::colored(message, PredefinedColor::Red));
    }
    
    pub fn command_names(&self) -> Vec<&'static str> {
        self.commands.keys().map(|command| *command).collect()
    }

    pub fn command_description(&self, command: &str) -> Option<&String> {
        self.descriptions.get(command)
    }
}

pub struct ParserState<'cmd> {
    raw_args: ArgumentTraverser<'cmd>,
    current_argument: Option<&'cmd str>,
    pause_traverser: bool,
    pub current_node: &'cmd CommandNode,
    pub stack: Vec<&'cmd CommandNode>
}

impl<'cmd> ParserState<'cmd> {
    pub fn new(raw_args: ArgumentTraverser<'cmd>, node: &'cmd CommandNode) -> Self {
        ParserState {
            raw_args,
            current_argument: None,
            pause_traverser: false,
            current_node: node,
            stack: vec![node]
        }
    }

    pub fn next_argument(&mut self) -> Option<&'cmd str> {
        if self.pause_traverser {
            self.pause_traverser = false;
        } else {
            self.current_argument = self.raw_args.next();
        }

        self.current_argument
    }

    pub fn node(&self, index: usize) -> Option<&'cmd CommandNode> {
        self.stack.get(index).map(|node| *node)
    }
}

#[derive(Clone)]
pub enum ParserRedirection<'cmd> {
    Node(&'cmd CommandNode),
    Root,
    None
}

impl<'cmd> From<Option<&'cmd CommandNode>> for ParserRedirection<'cmd> {
    fn from(option: Option<&'cmd CommandNode>) -> Self {
        match option {
            Some(node) => ParserRedirection::Node(node),
            None => ParserRedirection::None
        }
    }
}

#[derive(Clone)]
pub struct ExecutableCommand<'cmd>(&'cmd str);

impl<'cmd> ExecutableCommand<'cmd> {
    pub fn execute_with(&self, context: CommandContext<'cmd>) {
        context.executor.dispatch(self.0, context.server, context.sender);
    }
}

/// The context in which a command is executed. This has no use outside the lifecycle
/// of a command.
pub struct CommandContext<'cmd> {
    pub server: &'cmd QuartzServer,
    pub executor: &'cmd CommandExecutor,
    pub sender: CommandSender,
    pub arguments: HashMap<&'static str, Argument<'cmd>>
}

// Shortcut functions for getting argument values
impl<'cmd> CommandContext<'cmd> {
    pub fn new(server: &'cmd QuartzServer, executor: &'cmd CommandExecutor, sender: CommandSender) -> Self {
        CommandContext {
            server,
            executor,
            sender,
            arguments: HashMap::new()
        }
    }

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

    pub fn get_string(&self, key: &str) -> Option<&'cmd str> {
        match self.arguments.get(key) {
            Some(Argument::String(value)) => Some(value),
            _ => None
        }
    }

    pub fn get_command(&self, key: &str) -> Option<ExecutableCommand<'cmd>> {
        match self.arguments.get(key) {
            Some(Argument::Command(command)) => Some(command.clone()),
            _ => None
        }
    }
}

#[derive(Clone)]
pub struct NodeBase {
    name: &'static str,
    executor: Option<fn(CommandContext)>
}

impl NodeBase {
    fn new(name: &'static str) -> Self {
        NodeBase {
            name,
            executor: None
        }
    }
}

#[derive(Clone)]
pub enum CommandNode {
    Literal {
        base: NodeBase,
        children: Vec<CommandNode>
    },
    Argument {
        base: NodeBase,
        children: Vec<CommandNode>,
        argument: Argument<'static>,
        suggester: Option<fn(&CommandContext, &str) -> Vec<String>>,
        default: bool
    },
    Redirection {
        base: NodeBase,
        selector: for<'cmd> fn(&ParserState<'cmd>) -> ParserRedirection<'cmd>
    }
}

impl CommandNode {
    fn literal(name: &'static str) -> CommandNode {
        CommandNode::Literal {
            base: NodeBase::new(name),
            children: Vec::new()
        }
    }

    fn argument(name: &'static str, argument: Argument<'static>, default: bool) -> Self {
        CommandNode::Argument {
            base: NodeBase::new(name),
            children: Vec::new(),
            argument,
            suggester: None,
            default
        }
    }

    fn redirection(name: &'static str, selector: for<'cmd> fn(&ParserState<'cmd>) -> ParserRedirection<'cmd>) -> Self {
        CommandNode::Redirection {
            base: NodeBase::new(name),
            selector
        }
    }

    fn name(&self) -> &'static str {
        match self {
            CommandNode::Literal {base, ..} => base.name,
            CommandNode::Argument {base, ..} => base.name,
            CommandNode::Redirection {base, ..} => base.name
        }
    }

    fn has_children(&self) -> bool {
        match self {
            CommandNode::Literal {children, ..} => !children.is_empty(),
            CommandNode::Argument {children, ..} => !children.is_empty(),
            CommandNode::Redirection {..} => false
        }
    }

    fn children(&self) -> Option<&Vec<CommandNode>> {
        match self {
            CommandNode::Literal {children, ..} => Some(&children),
            CommandNode::Argument {children, ..} => Some(&children),
            CommandNode::Redirection {..} => None
        }
    }

    fn children_mut(&mut self) -> &mut Vec<CommandNode> {
        match &mut *self {
            CommandNode::Literal {children, ..} => children,
            CommandNode::Argument {children, ..} => children,
            CommandNode::Redirection {..} => panic!("Redirection nodes cannot have children.")
        }
    }

    fn matches(&self, arg: &str) -> bool {
        match self {
            CommandNode::Literal {base, ..} => base.name.eq_ignore_ascii_case(arg),
            CommandNode::Argument {argument, ..} => argument.matches(arg),
            CommandNode::Redirection {..} => true
        }
    }

    fn partial_match(&self, arg: &str) -> bool {
        match self {
            CommandNode::Literal {base, ..} => base.name.starts_with(arg),
            CommandNode::Argument {argument, ..} => argument.partial_match(arg),
            CommandNode::Redirection {..} => true
        }
    }

    fn executor(&self) -> Option<fn(CommandContext)> {
        match self {
            CommandNode::Literal {base, ..} => base.executor,
            CommandNode::Argument {base, ..} => base.executor,
            CommandNode::Redirection {base, ..} => base.executor
        }
    }

    // TODO: Complete documentation
    /// Applies this node to the given context, potentially modifying the parser sate.
    fn apply<'cmd>(
        &'cmd self,
        state: &mut ParserState<'cmd>,
        context: &mut CommandContext<'cmd>,
        arg: &'cmd str
    ) -> Result<(), Component> {
        match self {
            CommandNode::Argument {base, argument, ..} => {
                argument.apply(context, base.name, arg)
                    .map_err(|e| Component::colored(format!("Invalid value for argument \"{}\": {}", base.name, e), PredefinedColor::Red))?;
            },
            CommandNode::Redirection {base, selector} => match selector(&*state) {
                ParserRedirection::Node(node) => {
                    state.pause_traverser = true;
                    state.current_node = node;
                    state.stack.push(node);
                    return Ok(());
                },
                ParserRedirection::Root => {
                    context.arguments.insert(base.name, Argument::Command(ExecutableCommand(state.raw_args.remaining())));
                },
                ParserRedirection::None => {
                    return Err(Component::colored("Internal command parser error: failed to redirect".to_owned(), PredefinedColor::Red));
                }
            },
            _ => {}
        }

        state.current_node = self;
        state.stack.push(self);
        Ok(())
    }

    // Adds a child
    pub fn then(mut self, child: CommandNode) -> Self {
        self.children_mut().push(child);
        self
    }

    pub fn any_then(mut self, child: CommandNode) -> Self {
        for current_child in self.children_mut().iter_mut() {
            current_child.children_mut().push(child.clone());
        }
        self
    }

    // Adds an executor
    pub fn executes(mut self, executor: fn(CommandContext)) -> Self {
        match &mut self {
            CommandNode::Literal {base, ..} => base.executor = Some(executor),
            CommandNode::Argument {base, ..} => base.executor = Some(executor),
            CommandNode::Redirection {base, ..} => base.executor = Some(executor)
        }

        self
    }

    // Attempts to execute the node with the given context. Returns whether or not an executor was called
    pub fn execute(&self, context: CommandContext) -> bool {
        match &self.executor() {
            Some(executor) => {
                executor(context);
                true
            },
            None => false
        }
    }

    pub fn suggests(mut self, sugg: fn(&CommandContext, &str) -> Vec<String>) -> Self {
        match &mut self {
            CommandNode::Argument {suggester, ..} => *suggester = Some(sugg),
            _ => {}
        }

        self
    }

    pub fn add_suggestions<'cmd>(&self, state: &ParserState<'cmd>, context: &CommandContext, arg: &str, suggestions: &mut Vec<String>) {
        match self {
            CommandNode::Literal {base, ..} => suggestions.push(base.name.to_owned()),
            CommandNode::Argument {suggester, ..} => match suggester {
                Some(suggester) => suggestions.extend(suggester(context, arg)),
                // TODO: Add default suggestions for argument node
                None => {}
            },
            CommandNode::Redirection {selector, ..} => match selector(state) {
                ParserRedirection::Node(node) => {
                    for child in node.children().into_iter().flatten().filter(|child| child.partial_match(arg)) {
                        child.add_suggestions(state, context, arg, suggestions);
                    }
                },
                ParserRedirection::Root => {
                    suggestions.extend(context.executor.commands.keys().filter(|cmd| cmd.starts_with(arg)).map(|cmd| (*cmd).to_owned()));
                },
                ParserRedirection::None => {}
            }
        }
    }
}

// A command literal, or an exact string such as "foo"
#[inline]
pub fn literal(literal: &'static str) -> CommandNode {
    CommandNode::literal(literal)
}

// An integer value, signed or unsigned, parsed as an i64
#[inline]
pub fn integer(name: &'static str) -> CommandNode {
    CommandNode::argument(name, Argument::Integer(0), false)
}

// An integer with a default value
#[inline]
pub fn integer_default(name: &'static str, default: i64) -> CommandNode {
    CommandNode::argument(name, Argument::Integer(default), true)
}

// A floating point value parsed as an f64
#[inline]
pub fn float(name: &'static str) -> CommandNode {
    CommandNode::argument(name, Argument::FloatingPoint(0.0), false)
}

// A floating point argument with a default value
#[inline]
pub fn float_default(name: &'static str, default: f64) -> CommandNode {
    CommandNode::argument(name, Argument::FloatingPoint(default), true)
}

// A string argument, which is essentially just the raw argument
#[inline]
pub fn string(name: &'static str) -> CommandNode {
    CommandNode::argument(name, Argument::String(""), false)
}

// A string argument with a default value
#[inline]
pub fn string_default(name: &'static str, default: &'static str) -> CommandNode {
    CommandNode::argument(name, Argument::String(default), true)
}

#[inline]
pub fn redirect(selector: for<'cmd> fn(&ParserState<'cmd>) -> ParserRedirection<'cmd>) -> CommandNode {
    CommandNode::redirection("<redirection>", selector)
}

#[inline]
pub fn redirect_root(name: &'static str) -> CommandNode {
    CommandNode::redirection(name, |_state| ParserRedirection::Root)
}

// After the given vec of args, look for the following node
pub fn after(mut args: Vec<CommandNode>, last: CommandNode) -> CommandNode {
    if args.is_empty() {
        return last;
    }

    // Unwrap is safe since the length is checked above
    args.last_mut().unwrap().children_mut().push(last);

    while args.len() > 1 {
        // Unwraps are safe since the length is checked by the while loop
        let node = args.pop().unwrap();
        args.last_mut().unwrap().children_mut().push(node);
    }

    // The while loop exits when there is one element left, so this is also safe
    args.pop().unwrap()
}
use std::collections::HashMap;
use chat::{
    Component,
    color::PredefinedColor
};
use crate::Registry;
use crate::command::arg::*;
use crate::command::CommandSender;
use crate::QuartzServer;

/// Contains a map of commands and their descriptions
pub struct CommandExecutor<R: Registry> {
    commands: HashMap<&'static str, (CommandNode<R>, String)>
}

impl<R: Registry> CommandExecutor<R> {
    /// Creates a new command executor with an empty internal map.
    pub fn new() -> Self {
        CommandExecutor {
            commands: HashMap::new()
        }
    }

    /// Registers the given command node with its defined syntax. If the node is not a literal,
    /// then this function currently just ignores it and does nothing.
    pub fn register(&mut self, node: CommandNode<R>, description: &str) {
        match &node {
            CommandNode::Literal {base, ..} => {
                let name = base.name;
                self.commands.insert(name, (node, description.to_owned()));
            },
            // Perhaps consider handling this error
            _ => {}
        }
    }

    /// Attempts to dispatch the given command, first attempting to parse it according to the registered command
    /// syntax trees, and then applying an execution context to a valid stack generated from the tree. If an error
    /// occurs at some point, then the sender is notified.
    pub fn dispatch(&self, command: &str, server: &mut QuartzServer<R>, sender: CommandSender) {
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
            Some(root) => &root.0,
            None => {
                context.sender.send_message(
                    Component::colored(format!("No command found named \"{}\"", root_name), PredefinedColor::Red)
                );
                return;
            }
        };

        // Create the parser state with the arguments and root node
        let mut state = ParserState::new(raw_args, root);

        // Iterate over the avilable arguments and trace a path on the command tree
        'arg_loop: while let Some(arg) = state.raw_args.next() {
            // Find the next node in the tree
            if state.current_node.has_children() {
                let children = match state.current_node.children() {
                    Some(children) => children,
                    // This branch should not be taken due to the check above
                    None => return
                };

                // Find a child that matches the given argument and attempt to apply it
                for child in children.iter().filter(|child| child.matches(arg)) {
                    match child.apply(&mut state, &mut context, arg) {
                        Ok(()) => {
                            // Only use the first child that matches
                            continue 'arg_loop;
                        },

                        // The argument could not be applied due to a parse error, so notify the sender and exit
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
            // the sender that some arguments are getting ignored and execute the command
            else {
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

        // Look for, and load in default argument values
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
            break;
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

    /// Generates a list of suggestions based off the given command string. This method will traverse the command
    /// syntax trees identically to the dispatch method until the final argument is reached, at which point the
    /// last node on the stack is used to generate suggestions based off the partial argument.
    /// 
    /// If no arguments are present in the given command string, then the full list of commands is returned. If
    /// one argument is present, then the suggestions are generated based off the available commands.
    pub fn get_suggestions(&self, command: &str, server: &mut QuartzServer<R>, sender: CommandSender) -> Vec<String> {
        let mut context = CommandContext::new(server, self, sender);
        let mut raw_args = ArgumentTraverser::new(command);

        // Find the name of the command
        let root_name = match raw_args.next() {
            Some(arg) => arg,
            // Empty command, so return all available commands
            None => return self.commands.keys().map(|cmd| (*cmd).to_owned()).collect()
        };

        // Partial command name, so filter based off the given command string
        if !raw_args.has_next() {
            return self.commands.keys().filter(|cmd| cmd.starts_with(root_name)).map(|cmd| (*cmd).to_owned()).collect();
        }

        let root = match self.commands.get(root_name) {
            Some(root) => &root.0,
            None => return Vec::new()
        };

        // Create the parser state with the arguments and root node
        let mut state = ParserState::new(raw_args, root);

        // Iterate over the avilable arguments and trace a path on the command tree
        'arg_loop: while let Some(arg) = state.raw_args.next() {
            // Find the next node in the tree
            if state.current_node.has_children() {
                let children = match state.current_node.children() {
                    Some(children) => children,
                    // This branch should never be taken due to the check above
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

                            // Parse error
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
                Some(command) => self.get_suggestions(command.0, context.server, context.sender),
                None => Vec::new()
            },
            _ => Vec::new()
        }
    }

    /// We were expecting more arguments, so notify the sender what was expected.
    fn expect_args(children: &[CommandNode<R>], context: &CommandContext<'_, R>) {
        let mut message = "Expected one of the following arguments: ".to_owned();
        message.push_str(&children.iter().map(|child| child.name()).collect::<Vec<&'static str>>().join(", "));
        context.sender.send_message(Component::colored(message, PredefinedColor::Red));
    }
    
    /// Returns a vec of the registered command names.
    pub fn command_names(&self) -> Vec<&'static str> {
        self.commands.keys().map(|command| *command).collect()
    }

    /// Returns the description for the command with the given name, or `None` if no such command exists.
    pub fn command_description(&self, command: &str) -> Option<&String> {
        self.commands.get(command).map(|(_cmd, description)| description)
    }
}

/// Encapsulates the internal state of the parser, including the argument traverser, current node
/// and node stack.
pub struct ParserState<'cmd, R: Registry> {
    raw_args: ArgumentTraverser<'cmd>,
    /// The current node the parser is on.
    pub current_node: &'cmd CommandNode<R>,
    /// The record of visited nodes.
    pub stack: Vec<&'cmd CommandNode<R>>
}

impl<'cmd, R: Registry> ParserState<'cmd, R> {
    /// Creates a new parser sate with the given traverser and initial node. The given node is placed at
    /// the bottom of the stack.
    pub fn new(raw_args: ArgumentTraverser<'cmd>, node: &'cmd CommandNode<R>) -> Self {
        ParserState {
            raw_args,
            current_node: node,
            stack: vec![node]
        }
    }

    /// Returns the node on the stack at the given index, or `None` if the index is out of bounds.
    pub fn node(&self, index: usize) -> Option<&'cmd CommandNode<R>> {
        self.stack.get(index).map(|node| *node)
    }
}

/// Defines a way in which the parser is redirected to another node on the tree. A value of `None`
/// implies that there was a redirection error, and should only be used in that context.
#[derive(Clone)]
pub enum ParserRedirection<'cmd, R: Registry> {
    /// A redirection to another node on the parser stack.
    Node(
        /// The node on the stack.
        &'cmd CommandNode<R>
    ),
    /// Directs the parser to close off the current stack and push the remaining portion of the
    /// command into the arguments map, wrapped in an `ExecutableCommand`. This semantically implies
    /// that any valid command could follow in the remaining arguments.
    Root,
    /// Used to define a failure in parser redirection. If this value is encountered then the parser
    /// immediately terminates.
    None
}

impl<'cmd, R: Registry> From<Option<&'cmd CommandNode<R>>> for ParserRedirection<'cmd, R> {
    fn from(option: Option<&'cmd CommandNode<R>>) -> Self {
        match option {
            Some(node) => ParserRedirection::Node(node),
            None => ParserRedirection::None
        }
    }
}

/// A wrapper around command.
#[derive(Clone)]
#[repr(transparent)]
pub struct ExecutableCommand<'cmd>(&'cmd str);

impl<'cmd> ExecutableCommand<'cmd> {
    /// Dispatches the wrapped command using parameters from the given context.
    #[inline]
    pub fn execute_with<R: Registry>(&self, context: CommandContext<'cmd, R>) {
        context.executor.dispatch(self.0, context.server, context.sender);
    }
}

/// The context in which a command is executed. This has no use outside the lifecycle of a command.
pub struct CommandContext<'cmd, R: Registry> {
    /// A shared reference to the server.
    pub server: &'cmd mut QuartzServer<R>,
    /// A shared reference to the executor that created this context.
    pub executor: &'cmd CommandExecutor<R>,
    /// The sender of the command.
    pub sender: CommandSender,
    /// The parsed command arguments.
    pub arguments: HashMap<&'static str, Argument<'cmd>>
}

// Shortcut functions for getting argument values
impl<'cmd, R: Registry> CommandContext<'cmd, R> {
    /// Creates a new command context with the given parameters.
    pub fn new(server: &'cmd mut QuartzServer<R>, executor: &'cmd CommandExecutor<R>, sender: CommandSender) -> Self {
        CommandContext {
            server,
            executor,
            sender,
            arguments: HashMap::new()
        }
    }

    /// Returns an integer with the given name if such an argument can be found.
    pub fn get_integer(&self, key: &str) -> Option<i64> {
        match self.arguments.get(key) {
            Some(Argument::Integer(value)) => Some(*value),
            _ => None
        }
    }

    /// Returns a float with the given name if such an argument can be found.
    pub fn get_float(&self, key: &str) -> Option<f64> {
        match self.arguments.get(key) {
            Some(Argument::FloatingPoint(value)) => Some(*value),
            _ => None
        }
    }

    /// Returns a string with the given name if such an argument can be found.
    pub fn get_string(&self, key: &str) -> Option<&'cmd str> {
        match self.arguments.get(key) {
            Some(Argument::String(value)) => Some(value),
            _ => None
        }
    }

    /// Returns an executable command with the given name if such a command can be found.
    pub fn get_command(&self, key: &str) -> Option<ExecutableCommand<'cmd>> {
        match self.arguments.get(key) {
            Some(Argument::Command(command)) => Some(command.clone()),
            _ => None
        }
    }
}

/// The base for all command nodes which includes a name and optional executor.
pub struct NodeBase<R: Registry> {
    name: &'static str,
    executor: Option<fn(CommandContext<'_, R>)>
}

impl<R: Registry> NodeBase<R> {
    fn new(name: &'static str) -> Self {
        NodeBase {
            name,
            executor: None
        }
    }
}

impl<R: Registry> Clone for NodeBase<R> {
    fn clone(&self) -> Self {
        NodeBase {
            name: self.name,
            executor: self.executor.clone()
        }
    }
}

/// The command node type which can be one of three variants: literal, argument, or redirection. Literals
/// are just an exact string segment such as `"foo"`. Arguments are parsable values of varying types (see
/// the `Argument` enum for more detail). Redirection nodes provide a `ParserRedirection` based on the
/// current parser state, either redirecting to another node on the stack or the "root" node which can be
/// any command, or `None` if a redirection error occurs.
pub enum CommandNode<R: Registry> {
    /// A string literal such as "foo".
    Literal {
        /// The basic components of this node.
        base: NodeBase<R>,
        /// The children of this node.
        children: Vec<CommandNode<R>>
    },
    /// An argument node, such as an integer value or position.
    Argument {
        /// The basic components of this node.
        base: NodeBase<R>,
        /// The children of this node.
        children: Vec<CommandNode<R>>,
        /// The argument type.
        argument: Argument<'static>,
        /// An option suggestion generator.
        suggester: Option<fn(&CommandContext<'_, R>, &str) -> Vec<String>>,
        /// Whether or not this argument has a default value.
        default: bool
    },
    /// A redirection node.
    Redirection {
        /// The basic components of this node.
        base: NodeBase<R>,
        /// The function which determines where the parser is redirected.
        selector: for<'cmd> fn(&ParserState<'cmd, R>) -> ParserRedirection<'cmd, R>
    }
}

impl<R: Registry> CommandNode<R> {
    fn literal(name: &'static str) -> Self {
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

    fn redirection(name: &'static str, selector: for<'cmd> fn(&ParserState<'cmd, R>) -> ParserRedirection<'cmd, R>) -> Self {
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

    fn children(&self) -> Option<&[CommandNode<R>]> {
        match self {
            CommandNode::Literal {children, ..} => Some(&children),
            CommandNode::Argument {children, ..} => Some(&children),
            CommandNode::Redirection {..} => None
        }
    }

    fn children_mut(&mut self) -> &mut Vec<CommandNode<R>> {
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

    fn executor(&self) -> Option<fn(CommandContext<'_, R>)> {
        match self {
            CommandNode::Literal {base, ..} => base.executor,
            CommandNode::Argument {base, ..} => base.executor,
            CommandNode::Redirection {base, ..} => base.executor
        }
    }

    /// Applies this node to the given context, potentially modifying the parser sate. Argument nodes will attempt to parse
    /// their value from the string argument and add it to the argument map in the context. Redirection nodes will send the
    /// parser to a different node, at which point more arguments and literals can be consumed. Literals have no effect.
    fn apply<'cmd>(
        &'cmd self,
        state: &mut ParserState<'cmd, R>,
        context: &mut CommandContext<'cmd, R>,
        arg: &'cmd str
    ) -> Result<(), Component> {
        match self {
            CommandNode::Argument {base, argument, ..} => {
                argument.apply(context, base.name, arg)
                    .map_err(|e| Component::colored(format!("Invalid value for argument \"{}\": {}", base.name, e), PredefinedColor::Red))?;
            },
            CommandNode::Redirection {base, selector} => match selector(state) {
                ParserRedirection::Node(node) => {
                    state.raw_args.pause();
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

    /// Adds a child to this node.
    /// 
    /// # Panics
    /// Panics if called on a redirection node.
    pub fn then(mut self, child: CommandNode<R>) -> Self {
        self.children_mut().push(child);
        self
    }

    /// Adds the given node as a child of all children of this node.
    /// 
    /// # Panics
    /// Panics if called on a redirection node.
    pub fn any_then(mut self, child: CommandNode<R>) -> Self {
        for current_child in self.children_mut().iter_mut() {
            current_child.children_mut().push(child.clone());
        }
        self
    }

    /// Adds an executor to this node.
    pub fn executes(mut self, executor: fn(CommandContext<'_, R>)) -> Self {
        match &mut self {
            CommandNode::Literal {base, ..} => base.executor = Some(executor),
            CommandNode::Argument {base, ..} => base.executor = Some(executor),
            CommandNode::Redirection {base, ..} => base.executor = Some(executor)
        }

        self
    }

    /// Attempts to execute the node with the given context and returns whether or not an executor was called.
    pub fn execute(&self, context: CommandContext<'_, R>) -> bool {
        match &self.executor() {
            Some(executor) => {
                executor(context);
                true
            },
            None => false
        }
    }

    /// Adds a suggestion generator to this node. This method has no effect if not called on an argument
    /// node.
    pub fn suggests(mut self, sugg: fn(&CommandContext<'_, R>, &str) -> Vec<String>) -> Self {
        match &mut self {
            CommandNode::Argument {suggester, ..} => *suggester = Some(sugg),
            _ => {}
        }

        self
    }

    /// Adds suggestions to the given list based off of the given state variables and this node's type.
    pub fn add_suggestions<'cmd>(&self, state: &ParserState<'cmd, R>, context: &CommandContext<'_, R>, arg: &str, suggestions: &mut Vec<String>) {
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

impl<R: Registry> Clone for CommandNode<R> {
    fn clone(&self) -> Self {
        match self {
            CommandNode::Literal {base, children} => CommandNode::Literal {
                base: base.clone(),
                children: children.clone()
            },
            CommandNode::Argument {base, children, argument, suggester, default} => CommandNode::Argument {
                base: base.clone(),
                children: children.clone(),
                argument: argument.clone(),
                suggester: suggester.clone(),
                default: *default
            },
            CommandNode::Redirection {base, selector} => CommandNode::Redirection {
                base: base.clone(),
                selector: *selector
            }
        }
    }
}

/// A command literal, or an exact string such as "foo".
#[inline]
pub fn literal<R: Registry>(literal: &'static str) -> CommandNode<R> {
    CommandNode::literal(literal)
}

/// An integer value, signed or unsigned, parsed as an i64.
#[inline]
pub fn integer<R: Registry>(name: &'static str) -> CommandNode<R> {
    CommandNode::argument(name, Argument::Integer(0), false)
}

/// An integer with a default value.
#[inline]
pub fn integer_default<R: Registry>(name: &'static str, default: i64) -> CommandNode<R> {
    CommandNode::argument(name, Argument::Integer(default), true)
}

/// A floating point value parsed as an f64.
#[inline]
pub fn float<R: Registry>(name: &'static str) -> CommandNode<R> {
    CommandNode::argument(name, Argument::FloatingPoint(0.0), false)
}

/// A floating point argument with a default value.
#[inline]
pub fn float_default<R: Registry>(name: &'static str, default: f64) -> CommandNode<R> {
    CommandNode::argument(name, Argument::FloatingPoint(default), true)
}

/// A string argument, which is essentially just the raw argument.
#[inline]
pub fn string<R: Registry>(name: &'static str) -> CommandNode<R> {
    CommandNode::argument(name, Argument::String(""), false)
}

/// A string argument with a default value.
#[inline]
pub fn string_default<R: Registry>(name: &'static str, default: &'static str) -> CommandNode<R> {
    CommandNode::argument(name, Argument::String(default), true)
}

/// A redirection to another node.
#[inline]
pub fn redirect<R: Registry>(selector: for<'cmd> fn(&ParserState<'cmd, R>) -> ParserRedirection<'cmd, R>) -> CommandNode<R> {
    CommandNode::redirection("<redirection>", selector)
}

/// A redirection to the root node, or any valid command syntax tree.
#[inline]
pub fn redirect_root<R: Registry>(name: &'static str) -> CommandNode<R> {
    CommandNode::redirection(name, |_state| ParserRedirection::Root)
}

/// Chains the given nodes together in the given order.
pub fn after<R: Registry>(mut args: Vec<CommandNode<R>>, last: CommandNode<R>) -> CommandNode<R> {
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
use crate::{command::CommandContext, server::RUNNING};
use quartz_chat::{color::PredefinedColor, TextComponentBuilder};
use quartz_commands::{self, module, CommandModule};
use std::sync::atomic::Ordering;

pub struct StaticCommandExecutor;

impl StaticCommandExecutor {
    pub fn new() -> Self {
        StaticCommandExecutor
    }
}

impl<'ctx> CommandModule<CommandContext<'ctx>> for StaticCommandExecutor {
    fn dispatch(
        &self,
        command: &str,
        context: CommandContext<'ctx>,
    ) -> Result<(), quartz_commands::Error> {
        (NativeCommandSet).dispatch(command, context)
    }

    fn get_suggestions(&self, command: &str, context: &CommandContext<'ctx>) -> Vec<String> {
        (NativeCommandSet).get_suggestions(command, context)
    }
}

module! {
    mod native_command_set;
    type Context<'ctx> = CommandContext<'ctx>;

    // TODO: fix `help` command
    // command help
    // where
    //     cmd: String
    // {
    //     root executes |ctx| {
    //         ctx.sender.send_message(Component::colored(
    //             "-- Command List --".to_owned(),
    //             PredefinedColor::Gold,
    //         ));

    //         let command_names = ctx.executor.command_names();

    //         for command in command_names {
    //             ctx.sender.send_message(Component::colored(
    //                 command.to_owned(),
    //                 PredefinedColor::Gray,
    //             ));
    //         }
    //         ctx.sender.send_message(Component::colored(
    //             "-- Use 'help [command]' to get more information --".to_owned(),
    //             PredefinedColor::Gold,
    //         ));

    //         Ok(())
    //     };

    //     cmd executes |ctx| {
    //         let command = ctx.get_string("command").unwrap_or("");
    //         let help_msg = match ctx.executor.command_description(&command) {
    //             Some(message) => message,
    //             None => {
    //                 ctx.sender.send_message(Component::colored(
    //                     format!("Command not found: \"{}\"", command),
    //                     PredefinedColor::Red,
    //                 ));
    //                 return;
    //             }
    //         };

    //         ctx.sender.send_message(
    //             TextComponentBuilder::new(format!("{}: ", command))
    //                 .predef_color(PredefinedColor::Gold)
    //                 .add()
    //                 .text(help_msg.to_owned())
    //                 .predef_color(PredefinedColor::White)
    //                 .build(),
    //         );

    //         Ok(())
    //     };

    //     cmd suggests |ctx, arg| {
    //         ctx.executor
    //             .command_names()
    //             .iter()
    //             .filter(|cmd| cmd.starts_with(arg))
    //             .map(|&cmd| cmd.to_owned())
    //             .collect()
    //     };
    // }

    command stop {
        root executes |_ctx| {
            let _ = RUNNING.compare_exchange(true, false, Ordering::Acquire, Ordering::Relaxed);
            Ok(())
        };
    }

    command tps {
        root executes |ctx| {
            let mspt = ctx.server.clock.mspt();
            let tps = ctx.server.clock.as_tps(mspt);
            let red: f32;
            let green: f32;

            // Shift from dark green to yellow
            if tps > 15.0 {
                green = 128.0 + 14.4 * (20.0 - tps);
                red = 40.0 * (20.0 - tps);
            }
            // Shift from yellow to light red
            else if tps > 10.0 {
                green = 200.0 - 40.0 * (15.0 - tps);
                red = 200.0 + 11.0 * (15.0 - tps);
            }
            // Shift from light red to dark red
            else if tps > 0.0 {
                green = 0.0;
                red = 255.0 - 15.5 * (10.0 - tps);
            }
            // If everything is working this should never run
            else {
                green = 128.0;
                red = 0.0;
            }

            ctx.sender.send_message(
                TextComponentBuilder::new("Server TPS: ".to_owned())
                    .predef_color(PredefinedColor::Gold)
                    .add()
                    .text(format!(
                        "{:.2} ({}%), {:.3} mspt",
                        tps,
                        ((tps / ctx.server.clock.max_tps()) * 100_f32) as u32,
                        mspt
                    ))
                    .custom_color(red as u8, green as u8, 0)
                    .build(),
            );

            Ok(())
        };
    }
}

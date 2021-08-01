use crate::{command::CommandContext, ServerClock, DIAGNOSTICS, RUNNING};
use quartz_chat::{color::PredefinedColor, Component, ComponentBuilder};
use quartz_commands::{self, module, CommandModule, Help};
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

// NOTE: in order for the help command to work every command needs to have a Help<'cmd> argument that when executed outputs its help message
module! {
    mod native_command_set;
    type Context<'ctx> = CommandContext<'ctx>;

    command help
    where
        cmd: String
        help: Help<'cmd>
    {
        root executes |ctx| {
            ctx.sender.send_message(&Component::colored(
                "-- Command List --".to_owned(),
                PredefinedColor::Gold,
            ));

            let command_names = ctx.executor.get_suggestions("", &ctx);

            for command in command_names {
                ctx.sender.send_message(&Component::colored(
                    command.to_owned(),
                    PredefinedColor::Gray,
                ));
            }
            ctx.sender.send_message(&Component::colored(
                "-- Use 'help [command]' to get more information --".to_owned(),
                PredefinedColor::Gold,
            ));

            Ok(())
        };

        cmd executes |ctx| {
            ctx.executor.dispatch(&format!("{} -h", cmd), ctx)
        };

        help executes |ctx| {
            ctx.sender.send_message(&Component::text("Gives information on commands"));
            Ok(())
        };

        cmd suggests |ctx, arg| {
            ctx.executor.get_suggestions("", &ctx)
        };
    }

    command stop where
        help: Help<'cmd> {
        root executes |_ctx| {
            let _ = RUNNING.compare_exchange(true, false, Ordering::Acquire, Ordering::Relaxed);
            Ok(())
        };

        help executes |ctx| {
            ctx.sender.send_message(&Component::text("Stops the server"));
            Ok(())
        }
    }

    command tps where
        help: Help<'cmd> {
        root executes |ctx| {
            let mspt = DIAGNOSTICS.lock().unwrap().mspt();
            let tps = ServerClock::as_tps(mspt);
            let red: f64;
            let green: f64;

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
                &ComponentBuilder::new()
                    .color(PredefinedColor::Gold)
                    .add_text("Server TPS: ")
                    .custom_color(red as u8, green as u8, 0)
                    .add_text(format!(
                        "{:.2} ({}%), {:.3} mspt",
                        tps,
                        ((tps / ServerClock::max_tps()) * 100.0) as u32,
                        mspt
                    ))
                    .build(),
            );

            Ok(())
        };

        help executes |ctx| {
            ctx.sender.send_message(&Component::text("Shows the server's TPS and MSPT"));
            Ok(())
        }
    }
}

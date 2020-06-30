use std::sync::atomic::{Ordering};
use log::info;

use crate::command::executor::*;
use chat::{
    Component,
    TextComponentBuilder,
    color::PredefinedColor
};
use crate::server::RUNNING;



pub fn init_commands(command_executor: &mut CommandExecutor) {
    info!("Registering commands");
    
    /* NOTE: Please keep commands in alphabetical order */
    
    command_executor.register(literal("help").executes(|ctx| {
        ctx.sender.send_message(Component::colored("-- Command List --".to_owned(), PredefinedColor::Gold));

        let command_names = ctx.executor.command_names();

        for command in command_names {
            ctx.sender.send_message(Component::colored(command.to_owned(), PredefinedColor::Gray));
        }
        ctx.sender.send_message(Component::colored("-- Use 'help [command]' to get more information --".to_owned(), PredefinedColor::Gold));
    }).then(string("command").executes(|ctx| {
        let command = ctx.get_string("command").unwrap_or("".to_owned());
        let help_msg = match ctx.executor.command_description(&command) {
            Some(message) => message,
            None => {
                ctx.sender.send_message(Component::colored(format!("Command not found: \"{}\"", command), PredefinedColor::Red));
                return;
            }
        };

        ctx.sender.send_message(
            TextComponentBuilder::new(format!("{}: ", command))
                .predef_color(PredefinedColor::Gold)
                .add()
                .text(help_msg.to_owned())
                .predef_color(PredefinedColor::White)
                .build().into()
        );
    })), "Lists all commands and can give descriptions");

    command_executor.register(literal("plugins").executes(|ctx| {
        let plugins_list = &ctx.server.plugin_manager.plugins;

        let mut message = TextComponentBuilder::new(format!("-- There are currently {} plugins loaded --", plugins_list.len()))
            .predef_color(PredefinedColor::Gold);
        for plugin in plugins_list {
            message = message.add()
                .text(format!("\n{}: ", plugin.name))
                .predef_color(PredefinedColor::Green)
                .add()
                .text(format!("v{}", plugin.version))
                .predef_color(PredefinedColor::Blue);
        }

        ctx.sender.send_message(message.build().into());
    }), "Lists the current plugins");

    command_executor.register(literal("stop").executes(|_ctx| {
        RUNNING.compare_and_swap(true, false, Ordering::SeqCst);
    }), "Shuts down the server");

    command_executor.register(literal("tps").executes(|ctx| {
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
                .build().into()
        );
    }), "Gets the current tps and mspt of the server");
}
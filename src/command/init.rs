use std::sync::atomic::{Ordering};
use std::collections::HashMap;
use log::{info, debug};

use crate::command::executor::*;
use crate::{unchecked_component, custom_color};
use crate::server::RUNNING;



pub fn init_commands(command_executor: &mut CommandExecutor) {
    info!("Registering commands");
    
    /* NOTE: Please keep commands in alphabetical order */
	
	command_executor.register(literal("help").executes(move |ctx| {
		ctx.sender.send_message(unchecked_component!("&(gold)-- Command List --"));

		let command_names = ctx.executor.get_command_names();

		for command in &command_names {
			ctx.sender.send_message(unchecked_component!(command));
		}
		ctx.sender.send_message(unchecked_component!("&(gold)-- Use 'help [command]' to get more information --"));
	}).then(string("command").executes(move |ctx| {
		let help_msg = ctx.executor.get_command_description(&ctx.get_string("command"));

		if help_msg.is_some() {
			let msg = help_msg.unwrap();
			ctx.sender.send_message(unchecked_component!("{}: &(gold){}", ctx.get_string("command"), msg));
		}
		else {
			ctx.sender.send_message(unchecked_component!("No command {} found", ctx.get_string("commmand")));
		}
	})), "Lists all commands and can give descriptions");

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

        ctx.sender.send_message(unchecked_component!(
            "&(gold)Server TPS: &({}){:.2} ({}%), {:.3} mspt",
            custom_color!(red, green, 0),
            tps,
            ((tps / ctx.server.clock.max_tps()) * 100_f32) as u32,
            mspt
        ));
    }), "Gets the current tps and mspt of the server");
    
    
}
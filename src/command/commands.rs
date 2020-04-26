use std::sync::atomic::{Ordering};

use crate::command::executor::*;
use crate::{unchecked_component, custom_color};
use crate::server::RUNNING;

pub fn init_commands(command_executor: &mut CommandExecutor) {
    command_executor.register("stop", executor(|_ctx| {
        RUNNING.compare_and_swap(true, false, Ordering::SeqCst);
    }));

    command_executor.register("tps", executor(|ctx| {
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
            "&(gold)Server TPS: &({}){:.2} ({:.3} mspt)",
            custom_color!(red, green, 0),
            tps,
            mspt
        ));
    }));
}
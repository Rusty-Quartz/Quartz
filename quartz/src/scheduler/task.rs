use std::sync::atomic::AtomicBool;

use crate::{util::map::Identify, QuartzServer};

pub trait Task {
    type Output;

    fn tick(&mut self, server: &mut QuartzServer) -> TaskState;

    fn complete(self, server: &mut QuartzServer) -> Self::Output;
}

pub enum TaskState {
    InProgress,
    Complete,
    Invalid,
}

pub struct Delayed<F> {
    ticks_remaining: u32,
    func: F,
}

impl<T, F: FnOnce(&mut QuartzServer) -> T> Delayed<F> {
    pub fn new(delay: u32, func: F) -> Self {
        Delayed {
            ticks_remaining: delay,
            func,
        }
    }
}

impl<T, F: FnOnce(&mut QuartzServer) -> T> Task for Delayed<F> {
    type Output = T;

    fn tick(&mut self, _server: &mut QuartzServer) -> TaskState {
        if self.ticks_remaining > 0 {
            self.ticks_remaining -= 1;
            TaskState::InProgress
        } else {
            TaskState::Complete
        }
    }

    fn complete(self, server: &mut QuartzServer) -> Self::Output {
        (self.func)(server)
    }
}

use linefeed::{DefaultTerminal, Interface};
use log::{error, info};
use once_cell::sync::OnceCell;
use smol::{Timer, lock::{Mutex, RwLock}};
use std::{
    fmt::Display,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use crate::{CommandExecutor, Config, Diagnostics, QuartzServer};

/// The state variable that controls whether or not the server and its various sub-processes are running.
/// If set to false then the server will gracefully stop.
pub(crate) static RUNNING: AtomicBool = AtomicBool::new(false);
static CONFIG: OnceCell<RwLock<Config>> = OnceCell::new();
static RAW_CONSOLE: OnceCell<Arc<Interface<DefaultTerminal>>> = OnceCell::new();
pub static DIAGNOSTICS: Mutex<Diagnostics> = Mutex::new(Diagnostics::new());
static COMMAND_EXECUTOR: OnceCell<CommandExecutor> = OnceCell::new();

/// Returns whether or not the server is running.
#[inline]
pub fn is_running() -> bool {
    RUNNING.load(Ordering::Acquire)
}

pub fn config() -> &'static RwLock<Config> {
    CONFIG.get().expect("Config not initialized yet")
}

pub fn raw_console() -> &'static Interface<DefaultTerminal> {
    &**RAW_CONSOLE.get().expect("Raw console not initialized")
}

pub unsafe fn raw_console_unchecked() -> &'static Interface<DefaultTerminal> {
    &**RAW_CONSOLE.get_unchecked()
}

pub fn display_to_console<T: Display>(message: &T) {
    match raw_console().lock_writer_erase() {
        Ok(mut writer) =>
            if let Err(e) = writeln!(writer, "{}", message) {
                error!("Failed to send message to console: {}", e);
            },
        Err(e) => error!("Failed to lock console interface: {}", e),
    }
}

pub fn command_executor() -> &'static CommandExecutor {
    COMMAND_EXECUTOR
        .get()
        .expect("Command executor not initialized")
}

pub fn run(config: Config, raw_console: Arc<Interface<DefaultTerminal>>) {
    CONFIG
        .set(RwLock::new(config))
        .ok()
        .expect("Config initialized before server was run.");
    RAW_CONSOLE
        .set(raw_console)
        .ok()
        .expect("Raw console initialized before server was run.");

    let mut server = QuartzServer::new();
    server.init();
    COMMAND_EXECUTOR
        .set(CommandExecutor::new())
        .ok()
        .expect("Command executor initialized unexpectedly");

    info!("Started server thread");

    smol::block_on(async {
        let mut clock = ServerClock::new(50);

        while RUNNING.load(Ordering::Acquire) {
            if let Some(mut guard) = DIAGNOSTICS.try_lock() {
                guard.microseconds_per_tick = clock.micros_ema;
            }

            clock.start();
            server.tick().await;
            clock.finish_tick().await;
        }
    });
}

/// Keeps track of the time each tick takes and regulates the server ticks per second (TPS).
pub struct ServerClock {
    micros_ema: f64,
    full_tick: Duration,
    time: Instant,
}

impl ServerClock {
    /// Creates a new clock with the given tick length in milliseconds.
    pub fn new(tick_length: u128) -> Self {
        ServerClock {
            micros_ema: 0.0,
            full_tick: Duration::from_millis(tick_length as u64),
            time: Instant::now(),
        }
    }

    /// Called at the start of a server tick.
    pub(crate) fn start(&mut self) {
        self.time = Instant::now();
    }

    /// The tick code has finished executing, so record the time and sleep if extra time remains.
    async fn finish_tick(&mut self) {
        let elapsed = self.time.elapsed();
        self.micros_ema = (99.0 * self.micros_ema + elapsed.as_micros() as f64) / 100.0;

        if elapsed.as_millis() < 50 {
            Timer::after(self.full_tick - elapsed).await;
        }
    }

    /// Converts a milliseconds pet tick value to ticks per second.
    #[inline]
    pub fn as_tps(mspt: f64) -> f64 {
        if mspt < 50.0 {
            1000.0 / 50.0
        } else {
            1000.0 / mspt
        }
    }

    /// The maximum tps the server will tick at.
    #[inline]
    pub fn max_tps() -> f64 {
        1000.0 / 50.0
    }
}

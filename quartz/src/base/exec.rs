use linefeed::{DefaultTerminal, Interface};
use log::{error, info};
use once_cell::sync::OnceCell;
use parking_lot::{Mutex, RwLock};
use std::{
    fmt::Display,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{runtime::Builder, task::LocalSet};

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
            if let Err(e) = writeln!(writer, "{message}") {
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

    let rt = Builder::new_multi_thread()
        .enable_all()
        .thread_name("main-tick-thread")
        .build()
        .expect("Failed to build main-tick runtime");
    let rt = Arc::new(rt);

    let mut server = QuartzServer::new(Arc::clone(&rt));
    server.init();
    COMMAND_EXECUTOR
        .set(CommandExecutor::new())
        .ok()
        .expect("Command executor initialized unexpectedly");

    info!("Started server thread");

    let local_set = LocalSet::new();
    local_set.block_on(&*rt, async move {
        let mut clock = ServerClock::new();

        while RUNNING.load(Ordering::Acquire) {
            if let Some(mut guard) = DIAGNOSTICS.try_lock() {
                guard.microseconds_per_tick = clock.micros_ema;
            }

            clock.start();
            server.tick().await;
            clock.finish_tick().await;
        }
    });

    drop(local_set);

    let rt = Arc::try_unwrap(rt);
    match rt {
        Ok(rt) => rt.shutdown_timeout(Duration::from_secs(5)),
        Err(_) => error!("Failed to reclaim ownership of runtime"),
    }
}

const FULL_TICK_LENGTH: u64 = 50;
const FULL_TICK: Duration = Duration::from_millis(FULL_TICK_LENGTH);

/// Keeps track of the time each tick takes and regulates the server ticks per second (TPS).
pub struct ServerClock {
    micros_ema: f64,
    time: Instant,
}

impl ServerClock {
    /// Creates a new clock with the given tick length in milliseconds.
    pub fn new() -> Self {
        ServerClock {
            micros_ema: 0.0,
            time: Instant::now(),
        }
    }

    /// Called at the start of a server tick.
    pub(crate) fn start(&mut self) {
        self.time = Instant::now();
    }

    /// The tick code has finished executing, so record the time and sleep if extra time remains.
    async fn finish_tick(&mut self) -> f64 {
        let elapsed = self.time.elapsed();
        let micros = elapsed.as_micros() as f64;
        self.micros_ema = (99.0 * self.micros_ema + micros) / 100.0;

        if elapsed < FULL_TICK {
            tokio::time::sleep(FULL_TICK - elapsed).await;
        }

        micros
    }

    /// Converts a milliseconds pet tick value to ticks per second.
    #[inline]
    pub fn as_tps(mspt: f64) -> f64 {
        if mspt < FULL_TICK_LENGTH as f64 {
            1000.0 / FULL_TICK_LENGTH as f64
        } else {
            1000.0 / mspt
        }
    }

    /// The maximum tps the server will tick at.
    #[inline]
    pub fn max_tps() -> f64 {
        1000.0 / FULL_TICK_LENGTH as f64
    }
}

impl Default for ServerClock {
    fn default() -> Self {
        Self::new()
    }
}

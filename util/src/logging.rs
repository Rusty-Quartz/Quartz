use std::{
    error::Error,
    fmt,
    fs::{read_dir, remove_file, rename, File},
    io,
    path::Path,
    sync::{Arc, Mutex as StdMutex},
    thread,
};

use flate2::{write::GzEncoder, Compression};

use log4rs::{
    append::{
        rolling_file::{
            policy::compound::{roll::Roll, trigger::Trigger, CompoundPolicy},
            LogFile,
            RollingFileAppender,
        },
        Append,
    },
    config::{Appender, Config, Root},
    encode::pattern::PatternEncoder,
    filter::{Filter, Response},
};

use log::*;

use chrono::prelude::*;

use linefeed::{terminal::DefaultTerminal, Interface};

#[cfg(unix)]
use termion::color;

const FILE_SIZE_LIMIT: u64 = 50_000_000;

#[cfg(debug_assertions)]
const LEVEL_FILTER: LevelFilter = LevelFilter::Debug;
#[cfg(not(debug_assertions))]
const LEVEL_FILTER: LevelFilter = LevelFilter::Info;

/// Configures the log4rs crate to replicate the logging system for official minecraft servers.
///
/// Console output is filtered so that only logs from the specified crate are accepted. Messages are
/// in the form `[HH:MM:SS Level]: message`. Messages are automatically colored based on the log level
/// and can be customly colored with the `termion` crate. If debug assertions are off, then logging events
/// on the debug level are blocked.
///
/// Logs will be recorded in a directory named `logs` in the form `yyyy-mm-dd-log#`. Logs are
/// compressed using GZ encoding.
pub fn init_logger(
    crate_filter: &str,
    console_interface: Arc<Interface<DefaultTerminal>>,
) -> Result<(), Box<dyn Error>>
{
    // Logs info to the console with colors and such
    let console = CustomConsoleAppender { console_interface };

    // Logs to log files
    let logfile = RollingFileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("[{d(%H:%M:%S)} {l}]: {m}\n")))
        .build(
            "logs/latest.log",
            Box::new(CompoundPolicy::new(
                Box::new(CustomLogTrigger::new(FILE_SIZE_LIMIT)),
                Box::new(CustomLogRoller::new()),
            )),
        )?;

    // Build the log4rs config
    let config = Config::builder()
        .appender(
            Appender::builder()
                .filter(Box::new(CrateFilter::new(crate_filter)))
                .build("console", Box::new(console)),
        )
        .appender(
            Appender::builder()
                .filter(Box::new(CrateFilter::new(crate_filter)))
                .build("logfile", Box::new(logfile)),
        )
        .build(
            Root::builder()
                .appender("console")
                .appender("logfile")
                .build(LEVEL_FILTER),
        )?;

    log4rs::init_config(config)?;

    Ok(())
}

/// This should be called directly before the main process exits. This function simply compresses the
/// current log file.
pub fn cleanup() {
    // There's no reason to handle an error here
    let _ = CustomLogRoller::new().roll_threaded(Path::new("./logs/latest.log"), false);
}

// Only allow logging from out crate
#[cfg(debug_assertions)]
struct CrateFilter {
    filter: String,
}

#[cfg(not(debug_assertions))]
struct CrateFilter;

impl CrateFilter {
    #[cfg(debug_assertions)]
    pub fn new(filter: &str) -> Self {
        CrateFilter {
            filter: filter.to_owned(),
        }
    }

    #[cfg(not(debug_assertions))]
    pub fn new(_filter: &str) -> Self {
        CrateFilter
    }
}

impl Filter for CrateFilter {
    #[cfg(debug_assertions)]
    fn filter(&self, record: &Record) -> Response {
        if record.level() != Level::Debug && record.level() != Level::Trace {
            return Response::Accept;
        }

        match record.module_path() {
            Some(path) =>
                if path.starts_with(&self.filter) {
                    Response::Accept
                } else {
                    Response::Reject
                },
            None => Response::Reject,
        }
    }

    #[cfg(not(debug_assertions))]
    fn filter(&self, _record: &Record) -> Response {
        Response::Neutral
    }
}

impl fmt::Debug for CrateFilter {
    fn fmt(&self, _f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        Ok(())
    }
}

// Custom implementation for a console logger so that it doesn't mangle the user's commands
struct CustomConsoleAppender {
    console_interface: Arc<Interface<DefaultTerminal>>,
}

impl Append for CustomConsoleAppender {
    #[cfg(unix)]
    fn append(&self, record: &Record) -> Result<(), Box<dyn Error + Sync + Send>> {
        let mut writer = self.console_interface.lock_writer_erase()?;
        match record.metadata().level() {
            Level::Error => write!(writer, "{}", color::Fg(color::Red))?,
            Level::Warn => write!(writer, "{}", color::Fg(color::LightYellow))?,
            Level::Debug => write!(writer, "{}", color::Fg(color::LightCyan))?,
            _ => write!(writer, "{}", color::Fg(color::Reset))?,
        }
        writeln!(
            writer,
            "[{} {}]: {}{}",
            Local::now().format("%H:%M:%S"),
            record.metadata().level(),
            record.args(),
            color::Fg(color::Reset)
        )?;
        Ok(())
    }

    #[cfg(not(unix))]
    fn append(&self, record: &Record) -> Result<(), Box<dyn Error + Sync + Send>> {
        let mut writer = self.console_interface.lock_writer_erase()?;
        writeln!(
            writer,
            "[{} {}]: {}",
            Local::now().format("%H:%M:%S"),
            record.metadata().level(),
            record.args()
        )?;
        Ok(())
    }

    fn flush(&self) {}
}

impl fmt::Debug for CustomConsoleAppender {
    fn fmt(&self, _f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        Ok(())
    }
}

// Custom implementation for the rollover trigger, activates when the log file gets too large or a new day starts
struct CustomLogTrigger {
    last_day: StdMutex<u32>,
    max_size: u64,
}

impl CustomLogTrigger {
    pub fn new(max_size: u64) -> Self {
        CustomLogTrigger {
            last_day: StdMutex::new(Local::now().ordinal()),
            max_size,
        }
    }
}

impl fmt::Debug for CustomLogTrigger {
    fn fmt(&self, _f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        Ok(())
    }
}

impl Trigger for CustomLogTrigger {
    fn trigger(&self, file: &LogFile) -> Result<bool, Box<dyn Error + Sync + Send>> {
        if let Ok(mut guard) = self.last_day.lock() {
            let current_day = Local::now().ordinal();
            if current_day != *guard {
                *guard = current_day;
                return Ok(true);
            }
        }

        Ok(file.len_estimate() > self.max_size)
    }
}

struct CustomLogRoller {
    name_info: StdMutex<(u32, u32)>, // current day, log count for today
}

impl CustomLogRoller {
    pub fn new() -> Self {
        let mut max_index = 0;

        if let Ok(paths) = read_dir("./logs/") {
            let today = format!("{}", Local::now().format("%Y-%m-%d"));

            // Find the logs that match today's date and determine the highest index ({date}-{index}.log).
            for path in paths
                .flatten()
                .map(|entry| entry.file_name().into_string())
                .flatten()
                .filter(|name| name.starts_with(&today))
            {
                if let Some(index) = Self::index_from_path(&path) {
                    if index > max_index {
                        max_index = index;
                    }
                }
            }
        }

        CustomLogRoller {
            name_info: StdMutex::new((Local::now().ordinal(), max_index)),
        }
    }

    fn index_from_path(path: &str) -> Option<u32> {
        let dash_index = path.rfind("-")?;
        let dot_index = path.find(".")?;
        if dash_index + 1 < dot_index {
            path[dash_index + 1 .. dot_index].parse::<u32>().ok()
        } else {
            None
        }
    }

    pub fn roll_threaded(
        &self,
        file: &Path,
        threaded: bool,
    ) -> Result<(), Box<dyn Error + Sync + Send>>
    {
        let mut guard = match self.name_info.lock() {
            Ok(g) => g,

            // Since the mutex is privately managed and errors are handled correctly, this shouldn't be an issue
            Err(_) => unreachable!("Logger mutex poisoned."),
        };

        // Check to make sure the log name info is still accurate
        let local_datetime = Local::now();
        if local_datetime.ordinal() != guard.0 {
            guard.0 = local_datetime.ordinal();
            guard.1 = 1;
        } else {
            guard.1 += 1;
        }

        // Rename the file in case it's large and will take a while to compress
        let log = "./logs/latest-tmp.log";
        rename(file, log)?;

        let output = format!(
            "./logs/{}-{}.log.gz",
            local_datetime.format("%Y-%m-%d"),
            guard.1
        );

        if threaded {
            thread::spawn(move || {
                Self::try_compress_log(log, &output);
            });
        } else {
            Self::try_compress_log(log, &output);
        }

        Ok(())
    }

    // Attempts compress_log and prints an error if it fails
    fn try_compress_log(input_path: &str, output_path: &str) {
        if let Err(_) = Self::compress_log(Path::new(input_path), Path::new(output_path)) {
            error!("Failed to compress log file");
        }
    }

    // Takes the source file and compresses it, writing to the output path. Removes the source when done.
    fn compress_log(input_path: &Path, output_path: &Path) -> Result<(), io::Error> {
        let mut input = File::open(input_path)?;
        let mut output = GzEncoder::new(File::create(output_path)?, Compression::default());
        io::copy(&mut input, &mut output)?;
        drop(output.finish()?);
        drop(input); // This needs to occur before file deletion on some OS's
        remove_file(input_path)
    }
}

impl Roll for CustomLogRoller {
    fn roll(&self, file: &Path) -> Result<(), Box<dyn Error + Sync + Send>> {
        self.roll_threaded(file, true)
    }
}

impl fmt::Debug for CustomLogRoller {
    fn fmt(&self, _f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        Ok(())
    }
}

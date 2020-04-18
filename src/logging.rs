use std::{error::Error, path::Path};
use std::sync::{Arc, Mutex as StdMutex};
use std::fmt;
use std::fs::{File, read_dir, rename, remove_file};
use std::thread;
use std::io;

use flate2::write::GzEncoder;
use flate2::Compression;

use log4rs::append::{
    Append,
    rolling_file::{
        LogFile,
        RollingFileAppender,
        policy::compound::{
            CompoundPolicy,
            roll::Roll,
            trigger::Trigger
        }
    }
};
use log4rs::encode::pattern::PatternEncoder;
use log4rs::config::{Appender, Config, Root};

use log::*;

use chrono::prelude::*;

use linefeed::Interface;
use linefeed::terminal::DefaultTerminal;

#[cfg(unix)]
use termion::color;

const FILE_SIZE_LIMIT: u64 = 50_000_000;

#[cfg(debug_assertions)]
const LEVEL_FILTER: LevelFilter = LevelFilter::Debug;
#[cfg(not(debug_assertions))]
const LEVEL_FILTER: LevelFilter = LevelFilter::Info;

// Sets up log4rs customized for the minecraft server
pub fn init_logger(console_interface: Arc<Interface<DefaultTerminal>>) -> Result<(), Box<dyn Error>> {
    // Logs info to the console with colors and such
    let console = CustomConsoleAppender {console_interface};

    // Logs to log files
    let logfile = RollingFileAppender::builder()
            .encoder(Box::new(PatternEncoder::new("[{d(%H:%M:%S)} {l}]: {m}\n")))
            .build("logs/latest.log", Box::new(CompoundPolicy::new(
                Box::new(CustomLogTrigger::new(FILE_SIZE_LIMIT)),
                Box::new(CustomLogRoller::new())
            )))?;
    
    // Build the log4rs config
    let config = Config::builder()
            .appender(Appender::builder().build("console", Box::new(console)))
            .appender(Appender::builder().build("logfile", Box::new(logfile)))
            .build(
                Root::builder()
                    .appender("console")
                    .appender("logfile")
                    .build(LEVEL_FILTER)
            )?;
    
    log4rs::init_config(config)?;
    
    Ok(())
}

// Called at the end of main, compresses the last log file
pub fn cleanup() {
    // There's no reason to handle an error here, and thanks to the jackass who decided to avoid calling
    // drop on the log4rs objects
    let _ = CustomLogRoller::new().roll_threaded(Path::new("./logs/latest.log"), false);
}

// Custom implementation for a console logger so that it doesn't mangle the user's commands
struct CustomConsoleAppender {
    console_interface: Arc<Interface<DefaultTerminal>>
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
        writeln!(writer, "[{} {}]: {}{}", Local::now().format("%H:%M:%S"), record.metadata().level(), record.args(), color::Fg(color::Reset))?;
        Ok(())
    }

    #[cfg(not(unix))]
    fn append(&self, record: &Record) -> Result<(), Box<dyn Error + Sync + Send>> {
        let mut writer = self.console_interface.lock_writer_erase()?;
        writeln!(writer, "[{} {}]: {}", Local::now().format("%H:%M:%S"), record.metadata().level(), record.args())?;
        Ok(())
    }

    fn flush(&self) { }
}

impl fmt::Debug for CustomConsoleAppender {
    fn fmt(&self, _f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        Ok(())
    }
}

// Custom implementation for the rollover trigger, activates when the log file gets too large or a new day starts
struct CustomLogTrigger {
    last_day: StdMutex<u32>,
    max_size: u64
}

impl CustomLogTrigger {
    pub fn new(max_size: u64) -> Self {
        CustomLogTrigger {
            last_day: StdMutex::new(Local::now().ordinal()),
            max_size
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
    name_info: StdMutex<(u32, u32)> // current day, log count for today
}

impl CustomLogRoller {
    pub fn new() -> Self {
        let mut max_index = 0;
        
        if let Ok(paths) = read_dir("./logs/") {
            let today = format!("{}", Local::now().format("%Y-%m-%d"));

            // Find the logs that match today's date and determine the highest index ({date}-{index}.log).
            // This is incredibly ugly, find a better way to do it.
            for path in paths.flatten().map(|entry| entry.file_name().into_string()).flatten().filter(|name| name.starts_with(&today)) {
                if let Some(dash_index) = path.rfind("-") {
                    if let Some(dot_index) = path.find(".") {
                        if dash_index + 1 < dot_index {
                            if let Ok(index) = path[dash_index + 1..dot_index].parse::<u32>() {
                                if index > max_index {
                                    max_index = index;
                                }
                            }
                        }
                    }
                }
            }
        }

        CustomLogRoller {
            name_info: StdMutex::new((Local::now().ordinal(), max_index))
        }
    }

    pub fn roll_threaded(&self, file: &Path, threaded: bool) -> Result<(), Box<dyn Error + Sync + Send>> {
        let mut guard = match self.name_info.lock() {
            Ok(g) => g,
            
            // Since the mutex is privately managed and errors are handled correctly, this shouldn't be an issue
            Err(_) => unreachable!("Logger mutex poisoned.")
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

        let output = format!("./logs/{}-{}.log.gz", local_datetime.format("%Y-%m-%d"), guard.1);

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
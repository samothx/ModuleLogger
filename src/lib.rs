//! A consumer for the log crate
//!
//! The crate implements a logger that allows module-wise configuration of logging through
//! configuration files or an API.
//!
//! Features
//! * Log output can be written to stdout, stderr, to file or to a memory buffer.
//! * Log output can be colored.
//! * Features can be set using a configuration file or the API
//!
//! The configuration file can be enabled by setting the environment variable ```LOG_CONFIG``` to the
//! path of the file. The configuration is specified in YAML format and allows to set the following
//! values. All values are optional.
//!
//! * default_level: The default log level, one of trace, debug, info, warn, error, defaults to info
//! * mod_level: A list of module name and log level pairs
//! * log_dest: One of stdout, stderr, stream, buffer, streamstdout, streamstderr, bufferstdout, bufferstderr.
//! * log_stream: The log file name for stream variants of log_dest
//! * color: one of ```true``` or ```false```
//! * brief_info: one of ```true``` or ```false```
//!
//! Sample:
//! ```yaml
//! log_level: warn
//! log_dest: streamstderr
//! log_stream: debug.log
//! color: true
//! brief_info: true
//! mod_level:
//!   'test_mod': debug
//!   'test_mod::test_test': trace
//! ```
//!

use chrono::Local;
use colored::*;
use log::{Log, Metadata, Record};
use regex::Regex;
use std::env;
use std::fs::File;
#[cfg(feature = "config")]
use std::fs::OpenOptions;
use std::io::{stderr, stdout, BufWriter, Write};
use std::mem;
use std::sync::{Arc, Mutex, Once};

//, BufWriter};
mod error;

use error::{Error, ErrorKind, Result};
use std::path::Path;

#[cfg(feature = "config")]
pub mod config;

#[cfg(feature = "config")]
pub use config::LogConfig;

mod logger_params;

pub use logger_params::LogDestination;
use logger_params::LoggerParams;

pub(crate) const DEFAULT_LOG_LEVEL: Level = Level::Info;

// cannot be STREAM !!
pub(crate) const DEFAULT_LOG_DEST: LogDestination = LogDestination::Stderr;

pub const NO_STREAM: Option<Box<dyn 'static + Write + Send>> = None;

#[cfg(feature = "config")]
use crate::config::LogConfigBuilder;
use crate::error::ToError;
pub use log::Level;

// TODO: implement size limit for memory buffer
// TODO: Drop initialise functions and rather use a set_config function that can repeatedly reset the configuration

/// The Logger struct holds a singleton containing all relevant information.
///
/// struct Logger has a private constructor. It is used via its static interface which will
/// instantiate a Logger or use an existing one.
#[derive(Clone)]
pub struct Logger {
    inner: Arc<Mutex<LoggerParams>>,
    module_re: Regex,
    exe_name: Option<String>,
}

impl Logger {
    /// Create a new Logger or retrieve the existing one.\
    /// The function is private, Logger is meant to be used via its static interface
    /// Any of the static functions will initialise a Logger instance
    fn new() -> Logger {
        static mut LOGGER: *const Logger = 0 as *const Logger;
        static ONCE: Once = Once::new();

        // dbg!("Logger::new: entered");

        let exe_name = match env::current_exe() {
            Ok(exe_name) => match exe_name.file_name() {
                Some(exe_name) => exe_name
                    .to_str()
                    .map(|name| name.to_owned().replace('-', "_")),
                None => None,
            },
            Err(_why) => None,
        };

        let logger = unsafe {
            ONCE.call_once(|| {
                let singleton = Logger {
                    module_re: Regex::new(r#"^([^:]+)::(.*)$"#).unwrap(),
                    inner: Arc::new(Mutex::new(LoggerParams::new(DEFAULT_LOG_LEVEL))),
                    exe_name,
                };

                // Put it in the heap so it can outlive this call
                LOGGER = mem::transmute(Box::new(singleton));
            });

            (*LOGGER).clone()
        };

        //  is initialised tests and sets the flag
        if !logger.inner.lock().unwrap().initialised() {
            // looks like we only just created it
            // look for LOG_CONFIG in ENV
            #[cfg(feature = "config")]
            if let Ok(config_path) = env::var("LOG_CONFIG") {
                // eprintln!("LOG_CONFIG={}", config_path);
                match LogConfigBuilder::from_file(&config_path) {
                    Ok(ref log_config) => match logger.int_set_log_config(log_config.build()) {
                        Ok(_res) => (),
                        Err(why) => {
                            eprintln!(
                                "Failed to apply log config from file: '{}', error: {:?}",
                                config_path, why
                            );
                        }
                    },
                    Err(why) => {
                        eprintln!(
                            "Failed to read log config from file: '{}', error: {:?}",
                            config_path, why
                        );
                    }
                }
            }

            // potential race condition here regarding max_level

            match log::set_boxed_logger(Box::new(logger.clone())) {
                Ok(_dummy) => (),
                Err(why) => {
                    dbg!(why);
                }
            }

            log::set_max_level(logger.inner.lock().unwrap().max_level().to_level_filter());
        }

        // dbg!("Logger::new: done");
        // Now we give out a copy of the data that is safe to use concurrently.
        logger
    }

    /// Flush the contents of log buffers
    pub fn flush() {
        Logger::new().flush();
    }

    /// create a default logger
    pub fn create() {
        let _logger = Logger::new();
    }

    /// Initialise a Logger with the given default log_level or modify the default log level of the
    /// existing logger
    pub fn set_default_level(log_level: Level) {
        let logger = Logger::new();
        let mut guarded_params = logger.inner.lock().unwrap();
        let last_max_level = *guarded_params.max_level();
        let max_level = guarded_params.set_default_level(log_level);

        if last_max_level != max_level {
            log::set_max_level(max_level.to_level_filter());
        }
    }

    /// Retrieve the default level of the logger
    pub fn get_default_level(&self) -> Level {
        let guarded_params = self.inner.lock().unwrap();
        guarded_params.get_default_level()
    }

    /// Modify the log level for a module
    pub fn set_mod_level(module: &str, log_level: Level) {
        let logger = Logger::new();
        let mut guarded_params = logger.inner.lock().unwrap();
        let last_max_level = *guarded_params.max_level();
        let max_level = guarded_params.set_mod_level(module, log_level);
        if last_max_level != *max_level {
            log::set_max_level(max_level.to_level_filter());
        }
    }

    /// Retrieve the current log buffer, if available
    pub fn get_buffer() -> Option<Vec<u8>> {
        let logger = Logger::new();
        let mut guarded_params = logger.inner.lock().unwrap();
        guarded_params.retrieve_log_buffer()
    }

    /// Set the log destination
    pub fn set_log_dest<S: 'static + Write + Send>(
        dest: &LogDestination,
        stream: Option<S>,
    ) -> Result<()> {
        let logger = Logger::new();
        logger.flush();
        let mut guarded_params = logger.inner.lock().unwrap();
        guarded_params.set_log_dest(dest, stream)
    }

    /// Set log destination  and log file.
    pub fn set_log_file(log_dest: &LogDestination, log_file: &Path, buffered: bool) -> Result<()> {
        let dest = if log_dest.is_stdout() {
            LogDestination::StreamStdout
        } else if log_dest.is_stderr() {
            LogDestination::StreamStderr
        } else {
            LogDestination::Stream
        };

        let mut stream: Box<dyn Write + Send> = if buffered {
            Box::new(BufWriter::new(
                File::create(log_file).upstream_with_context(&format!(
                    "Failed to create file: '{}'",
                    log_file.display()
                ))?,
            ))
        } else {
            Box::new(File::create(log_file).upstream_with_context(&format!(
                "Failed to create file: '{}'",
                log_file.display()
            ))?)
        };

        let logger = Logger::new();
        logger.flush();

        let mut guarded_params = logger.inner.lock().unwrap();
        let buffer = guarded_params.retrieve_log_buffer();

        if let Some(buffer) = buffer {
            stream
                .write_all(buffer.as_slice())
                .upstream_with_context(&format!(
                    "Failed to write buffers to file: '{}'",
                    log_file.display()
                ))?;
            stream.flush().upstream_with_context(&format!(
                "Failed to flush buffers to file: '{}'",
                log_file.display()
            ))?;
        }

        guarded_params.set_log_dest(&dest, Some(stream))
    }

    /// Retrieve the current log destination
    pub fn get_log_dest() -> LogDestination {
        let logger = Logger::new();
        let guarded_params = logger.inner.lock().unwrap();
        guarded_params.get_log_dest().clone()
    }

    /// Set the log configuration.
    #[cfg(feature = "config")]
    pub fn set_log_config(log_config: &LogConfig) -> Result<()> {
        Logger::new().int_set_log_config(log_config)
    }

    /// Enable / disable colored output
    pub fn set_color(color: bool) {
        let logger = Logger::new();
        let mut guarded_params = logger.inner.lock().unwrap();
        guarded_params.set_color(color)
    }

    /// Enable / disable timestamp in messages
    pub fn set_timestamp(val: bool) {
        let logger = Logger::new();
        let mut guarded_params = logger.inner.lock().unwrap();
        guarded_params.set_timestamp(val)
    }

    /// Enable / disable timestamp in messages
    pub fn set_millis(val: bool) {
        let logger = Logger::new();
        let mut guarded_params = logger.inner.lock().unwrap();
        guarded_params.set_millis(val)
    }

    /// Enable / disable brief info messages
    pub fn set_brief_info(val: bool) {
        let logger = Logger::new();
        let mut guarded_params = logger.inner.lock().unwrap();
        guarded_params.set_brief_info(val)
    }

    #[cfg(feature = "config")]
    fn int_set_log_config(&self, log_config: &LogConfig) -> Result<()> {
        let mut guarded_params = self.inner.lock().unwrap();
        let last_max_level = *guarded_params.max_level();

        guarded_params.set_default_level(log_config.get_default_level());

        let max_level = guarded_params.set_mod_config(log_config.get_mod_level());
        if max_level != &last_max_level {
            log::set_max_level(max_level.to_level_filter());
        }

        let log_dest = guarded_params.get_log_dest();
        let cfg_log_dest = log_config.get_log_dest();
        let stream_log = cfg_log_dest.is_stream_dest();

        if cfg_log_dest != log_dest || stream_log {
            if stream_log {
                if let Some(log_stream) = log_config.get_log_stream() {
                    guarded_params.set_log_dest(
                        cfg_log_dest,
                        Some(
                            OpenOptions::new()
                                .append(true)
                                .create(true)
                                .open(log_stream)
                                .upstream_with_context(&format!(
                                    "Failed to open log file: '{}'",
                                    log_stream.display()
                                ))?,
                        ),
                    )?;
                } else {
                    return Err(Error::with_context(
                        ErrorKind::InvParam,
                        &format!(
                            "Missing parameter log_stream for destination {:?}",
                            cfg_log_dest
                        ),
                    ));
                }
            } else {
                guarded_params.set_log_dest(cfg_log_dest, NO_STREAM)?;
            }
        }

        guarded_params.set_color(log_config.is_color());
        guarded_params.set_brief_info(log_config.is_brief_info());

        Ok(())
    }
}

impl Log for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        let (mod_name, mod_tag) = if let Some(mod_path) = record.module_path() {
            if let Some(ref exe_name) = self.exe_name {
                if let Some(ref captures) = self.module_re.captures(mod_path) {
                    if captures.get(1).unwrap().as_str() == exe_name {
                        (
                            mod_path.to_owned(),
                            captures.get(2).unwrap().as_str().to_owned(),
                        )
                    } else {
                        (mod_path.to_owned(), mod_path.to_owned())
                    }
                } else if mod_path == exe_name {
                    (mod_path.to_owned(), String::from("main"))
                } else {
                    (mod_path.to_owned(), mod_path.to_owned())
                }
            } else {
                (mod_path.to_owned(), mod_path.to_owned())
            }
        } else {
            (String::from("undefined"), String::from("undefined"))
        };

        let curr_level = record.metadata().level();

        let mut guarded_params = self.inner.lock().unwrap();
        let mut level = guarded_params.get_default_level();
        if let Some(mod_level) = guarded_params.get_mod_level(&mod_tag) {
            level = mod_level;
        }

        if curr_level <= level {
            let timestamp = if guarded_params.timestamp() {
                let now = Local::now();
                if guarded_params.millis() {
                    let ts_millis = now.timestamp_millis() % 1000;
                    format!("{}.{:03} ", now.format("%Y-%m-%d %H:%M:%S"), ts_millis)
                } else {
                    format!("{} ", now.format("%Y-%m-%d %H:%M:%S"))
                }
            } else {
                "".to_owned()
            };

            let mut output = if guarded_params.brief_info() && (curr_level == Level::Info) {
                format!(
                    "{}{:<5} {}\n",
                    timestamp,
                    record.level().to_string(),
                    record.args()
                )
            } else {
                format!(
                    "{}{:<5} [{}] {}\n",
                    timestamp,
                    record.level().to_string(),
                    &mod_name,
                    record.args()
                )
            };

            if guarded_params.color() {
                output = match curr_level {
                    Level::Error => format!("{}", output.red()),
                    Level::Warn => format!("{}", output.yellow()),
                    Level::Info => format!("{}", output.green()),
                    Level::Debug => format!("{}", output.cyan()),
                    Level::Trace => format!("{}", output.blue()),
                };
            }

            let _res = match guarded_params.get_log_dest() {
                LogDestination::Stderr => stderr().write(output.as_bytes()),
                LogDestination::Stdout => stdout().write(output.as_bytes()),
                LogDestination::Stream => {
                    if let Some(ref mut stream) = guarded_params.log_stream() {
                        stream.write(output.as_bytes())
                    } else {
                        stderr().write(output.as_bytes())
                    }
                }
                LogDestination::StreamStdout => {
                    if let Some(ref mut stream) = guarded_params.log_stream() {
                        let _wres = stream.write(output.as_bytes());
                    }
                    stdout().write(output.as_bytes())
                }
                LogDestination::StreamStderr => {
                    if let Some(ref mut stream) = guarded_params.log_stream() {
                        let _wres = stream.write(output.as_bytes());
                    }
                    stderr().write(output.as_bytes())
                }
                LogDestination::Buffer => {
                    if let Some(ref mut buffer) = guarded_params.log_buffer() {
                        buffer.write(output.as_bytes())
                    } else {
                        stderr().write(output.as_bytes())
                    }
                }
                LogDestination::BufferStdout => {
                    if let Some(ref mut buffer) = guarded_params.log_buffer() {
                        let _wres = buffer.write(output.as_bytes());
                    }
                    stdout().write(output.as_bytes())
                }
                LogDestination::BufferStderr => {
                    if let Some(ref mut buffer) = guarded_params.log_buffer() {
                        let _wres = buffer.write(output.as_bytes());
                    }
                    stderr().write(output.as_bytes())
                }
            };
        }
    }

    fn flush(&self) {
        let mut guarded_params = self.inner.lock().unwrap();
        guarded_params.flush();
    }
}

/*
#[cfg(test)]
mod test {
    use log::{info};
    use crate::{Logger, LogDestination};
    #[test]
    fn log_to_mem() {
        Logger::initialise(Some("debug")).unwrap();
        let buffer: Vec<u8> = vec![];

        Logger::set_log_dest(&LogDestination::STREAM, Some(buffer)).unwrap();

        info!("logging to memory buffer");

        assert!(!buffer.is_empty());
    }
}
*/

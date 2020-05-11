use chrono::Local;
use colored::*;
use failure::ResultExt;
use log::{Log, Metadata, Record};
use regex::Regex;
use std::env;
use std::fs::{File, OpenOptions};
use std::io::{stderr, stdout, BufWriter, Write};
use std::mem;
use std::sync::{Arc, Mutex, Once}; //, BufWriter};
mod log_error;
use log_error::{LogErrCtx, LogError, LogErrorKind};
use std::path::Path;

pub mod config;
pub use config::LogConfig;

mod logger_params;
pub use logger_params::LogDestination;
use logger_params::LoggerParams;

pub(crate) const DEFAULT_LOG_LEVEL: Level = Level::Warn;

// cannot be STREAM !!
pub(crate) const DEFAULT_LOG_DEST: LogDestination = LogDestination::Stderr;

pub const NO_STREAM: Option<Box<dyn 'static + Write + Send>> = None;

use crate::config::LogConfigBuilder;
pub use log::Level;

// TODO: implement size limit for memory buffer
// TODO: Drop initialise functions and rather use a set_config function that can repeatedly reset the configuration

#[doc = " The mod_logger Log Consumer

A log consumer for the Log crate.

Implements a singleton holding the initialized LoggerParams
Using any of the static functions of the Logger interface will initialise a Logger
"]
#[derive(Clone)]
pub struct Logger {
    inner: Arc<Mutex<LoggerParams>>,
    module_re: Regex,
}

impl<'a> Logger {
    /// Create a new Logger or retrieve the existing one.\
    /// The function is private, Logger is meant to be used via its static interface
    /// Any of the static functions will initialise a Logger instance
    fn new() -> Logger {
        static mut LOGGER: *const Logger = 0 as *const Logger;
        static ONCE: Once = Once::new();

        // dbg!("Logger::new: entered");

        let logger = unsafe {
            ONCE.call_once(|| {
                // Make it
                //dbg!("call_once");
                let singleton = Logger {
                    module_re: Regex::new(r#"^[^:]+::(.*)$"#).unwrap(),
                    inner: Arc::new(Mutex::new(LoggerParams::new(DEFAULT_LOG_LEVEL))),
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
            if let Ok(config_path) = env::var("LOG_CONFIG") {
                match LogConfigBuilder::from_file(&config_path) {
                    Ok(ref log_config) => match logger.int_set_log_config(log_config.build()) {
                        Ok(_res) => {
                            dbg!("applied log config",);
                        }
                        Err(why) => {
                            dbg!(
                                "Failed to apply log config from file: '{}', error: {:?}",
                                config_path,
                                why
                            );
                        }
                    },
                    Err(why) => {
                        dbg!(
                            "Failed to read log config from file: '{}', error: {:?}",
                            config_path,
                            why
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

            log::set_max_level(
                logger
                    .inner
                    .lock()
                    .unwrap()
                    .get_max_level()
                    .to_level_filter(),
            );
        }

        // dbg!("Logger::new: done");
        // Now we give out a copy of the data that is safe to use concurrently.
        logger
    }

    /// Flush the contents of log buffers
    pub fn flush() {
        Logger::new().flush();
    }

    pub fn create() {
        let _logger = Logger::new();
    }

    /// Initialise a Logger with the given default log_level or modify the default log level of the
    /// existing logger
    pub fn set_default_level(log_level: Level) {
        let logger = Logger::new();
        let mut guarded_params = logger.inner.lock().unwrap();
        let last_max_level = *guarded_params.get_max_level();
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

    #[doc = "Modify the log level for a module"]
    pub fn set_mod_level(module: &str, log_level: Level) {
        let logger = Logger::new();
        let mut guarded_params = logger.inner.lock().unwrap();
        let last_max_level = *guarded_params.get_max_level();
        let max_level = guarded_params.set_mod_level(module, log_level);
        if last_max_level != max_level {
            log::set_max_level(max_level.to_level_filter());
        }
    }

    #[doc = "Retrieve the current log buffer, if available"]
    pub fn get_buffer() -> Option<Vec<u8>> {
        let logger = Logger::new();
        let mut guarded_params = logger.inner.lock().unwrap();
        guarded_params.retrieve_log_buffer()
    }

    #[doc = "Set the log destination"]
    pub fn set_log_dest<S: 'static + Write + Send>(
        dest: &LogDestination,
        stream: Option<S>,
    ) -> Result<(), LogError> {
        let logger = Logger::new();
        logger.flush();
        let mut guarded_params = logger.inner.lock().unwrap();
        guarded_params.set_log_dest(dest, stream)
    }

    pub fn set_log_file(
        log_dest: &LogDestination,
        log_file: &Path,
        buffered: bool,
    ) -> Result<(), LogError> {
        let dest = if log_dest.is_stdout() {
            LogDestination::StreamStdout
        } else if log_dest.is_stderr() {
            LogDestination::StreamStderr
        } else {
            LogDestination::Stream
        };

        let mut stream: Box<dyn Write + Send> = if buffered {
            Box::new(BufWriter::new(File::create(log_file).context(
                LogErrCtx::from_remark(
                    LogErrorKind::Upstream,
                    &format!("Failed to create file: '{}'", log_file.display()),
                ),
            )?))
        } else {
            Box::new(File::create(log_file).context(LogErrCtx::from_remark(
                LogErrorKind::Upstream,
                &format!("Failed to create file: '{}'", log_file.display()),
            ))?)
        };

        let logger = Logger::new();
        logger.flush();

        let mut guarded_params = logger.inner.lock().unwrap();
        let buffer = guarded_params.retrieve_log_buffer();

        if let Some(buffer) = buffer {
            stream
                .write_all(buffer.as_slice())
                .context(LogErrCtx::from_remark(
                    LogErrorKind::Upstream,
                    &format!("Failed to write buffers to file: '{}'", log_file.display()),
                ))?;
            stream.flush().context(LogErrCtx::from_remark(
                LogErrorKind::Upstream,
                &format!("Failed to flush buffers to file: '{}'", log_file.display()),
            ))?;
        }

        guarded_params.set_log_dest(&dest, Some(stream))
    }

    #[doc = "Retrieve the current log destination"]
    pub fn get_log_dest() -> LogDestination {
        let logger = Logger::new();
        let guarded_params = logger.inner.lock().unwrap();
        guarded_params.get_log_dest().clone()
    }

    #[doc = "Set the log configuration"]
    pub fn set_log_config(log_config: &LogConfig) -> Result<(), LogError> {
        Logger::new().int_set_log_config(log_config)
    }

    pub fn set_color(color: bool) {
        let logger = Logger::new();
        let mut guarded_params = logger.inner.lock().unwrap();
        guarded_params.set_color(color)
    }

    fn int_set_log_config(&self, log_config: &LogConfig) -> Result<(), LogError> {
        let mut guarded_params = self.inner.lock().unwrap();
        let last_max_level = *guarded_params.get_max_level();

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
                        Some(BufWriter::new(
                            OpenOptions::new()
                                .append(true)
                                .create(true)
                                .open(log_stream)
                                .context(LogErrCtx::from_remark(
                                    LogErrorKind::Upstream,
                                    &format!("Failed to open log file: '{}'", log_stream.display()),
                                ))?,
                        )),
                    )?;
                } else {
                    return Err(LogError::from_remark(
                        LogErrorKind::InvParam,
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

        Ok(())
    }
}

impl Log for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        // dbg!("Logger::log:");
        let (mod_name, mod_tag) = if let Some(mod_path) = record.module_path() {
            if let Some(ref captures) = self.module_re.captures(mod_path) {
                (
                    String::from(mod_path),
                    String::from(captures.get(1).unwrap().as_str()),
                )
            } else {
                (String::from(mod_path), String::from("main"))
            }
        } else {
            (String::from("undefined"), String::from("undefined"))
        };

        //dbg!(format!("mod_name: {} - mod_tag: {}", mod_name, mod_tag));

        let curr_level = record.metadata().level();

        let mut guarded_params = self.inner.lock().unwrap();
        let mut level = guarded_params.get_default_level();
        if let Some(mod_level) = guarded_params.get_mod_level(&mod_tag) {
            level = mod_level;
        }

        if curr_level <= level {
            let mut output = format!(
                "{} {:<5} [{}] {}\n",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level().to_string(),
                &mod_name,
                record.args()
            );

            if guarded_params.is_color() {
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
                    if let Some(ref mut stream) = guarded_params.get_log_stream() {
                        stream.write(output.as_bytes())
                    } else {
                        stderr().write(output.as_bytes())
                    }
                }
                LogDestination::StreamStdout => {
                    if let Some(ref mut stream) = guarded_params.get_log_stream() {
                        let _wres = stream.write(output.as_bytes());
                    }
                    stdout().write(output.as_bytes())
                }
                LogDestination::StreamStderr => {
                    if let Some(ref mut stream) = guarded_params.get_log_stream() {
                        let _wres = stream.write(output.as_bytes());
                    }
                    stderr().write(output.as_bytes())
                }
                LogDestination::Buffer => {
                    if let Some(ref mut buffer) = guarded_params.get_log_buffer() {
                        buffer.write(output.as_bytes())
                    } else {
                        stderr().write(output.as_bytes())
                    }
                }
                LogDestination::BufferStdout => {
                    if let Some(ref mut buffer) = guarded_params.get_log_buffer() {
                        let _wres = buffer.write(output.as_bytes());
                    }
                    stdout().write(output.as_bytes())
                }
                LogDestination::BufferStderr => {
                    if let Some(ref mut buffer) = guarded_params.get_log_buffer() {
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

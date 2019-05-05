use chrono::Local;
use colored::*;
use failure::ResultExt;
use log::{Level, Log, Metadata, Record};
use regex::Regex;
use std::env;
use std::fs::File;
use std::io::{stderr, stdout, BufWriter, Write};
use std::mem;
use std::str::FromStr;
use std::sync::{Arc, Mutex, Once, ONCE_INIT}; //, BufWriter};
mod log_error;
use log_error::{LogErrCtx, LogError, LogErrorKind};

mod config;
use config::LogConfig;

mod logger_params;
use logger_params::{LogDestination, LoggerParams};

pub(crate) const DEFAULT_LOG_LEVEL: Level = Level::Warn;

// cannot be STREAM !!
pub(crate) const DEFAULT_LOG_DEST: LogDestination = LogDestination::STDERR;

const NO_STREAM: Option<Box<'static + Write + Send>> = None;

#[derive(Clone)]
pub struct Logger {
    inner: Arc<Mutex<LoggerParams>>,
    module_re: Regex,
}

impl<'a> Logger {
    pub fn new() -> Logger {
        static mut SINGLETON: *const Logger = 0 as *const Logger;
        static ONCE: Once = ONCE_INIT;

        unsafe {
            ONCE.call_once(|| {
                // Make it
                let singleton = Logger {
                    module_re: Regex::new(r#"^[^:]+::(.*)$"#).unwrap(),
                    inner: Arc::new(Mutex::new(LoggerParams::new(DEFAULT_LOG_LEVEL))),
                };

                // Put it in the heap so it can outlive this call
                SINGLETON = mem::transmute(Box::new(singleton));
            });

            // Now we give out a copy of the data that is safe to use concurrently.
            (*SINGLETON).clone()
        }
    }

    pub fn set_default_level(log_level: &Level) {
        let logger = Logger::new();
        let mut guarded_params = logger.inner.lock().unwrap();
        guarded_params.set_default_level(log_level);
    }

    pub fn get_default_level(&self) -> Level {
        let guarded_params = self.inner.lock().unwrap();
        guarded_params.get_default_level().clone()
    }

    pub fn set_mod_level(module: &str, log_level: &Level) {
        let logger = Logger::new();
        let mut guarded_params = logger.inner.lock().unwrap();
        guarded_params.set_mod_level(module, log_level);
    }

    pub fn set_log_dest<S: 'static + Write + Send>(
        dest: &LogDestination,
        stream: Option<S>,
    ) -> Result<(), LogError> {
        let logger = Logger::new();
        logger.flush();
        let mut guarded_params = logger.inner.lock().unwrap();
        guarded_params.set_log_dest(dest, stream)
    }

    pub fn set_log_config(log_config: &LogConfig) -> Result<(), LogError> {
        let logger = Logger::new();
        let mut guarded_params = logger.inner.lock().unwrap();
        let last_max_level = guarded_params.get_max_level().clone();
        
        if let Some(ref default_level) = log_config.default_level {
            guarded_params.set_default_level(default_level);
        }

        let max_level = guarded_params.set_mod_config(&log_config.mod_level);
        if max_level != &last_max_level {
            log::set_max_level(max_level.to_level_filter());
        }

        let log_dest = guarded_params.get_log_dest();
        if &log_config.log_dest != log_dest || log_dest == &LogDestination::STREAM {
            if log_config.log_dest == LogDestination::STREAM {
                if let Some(ref log_path) = log_config.log_stream {
                    let stream = BufWriter::new(File::open(log_path).context(
                        LogErrCtx::from_remark(
                            LogErrorKind::Upstream,
                            &format!("Failed to open log file: '{}'", log_path.display()),
                        ),
                    )?);
                    guarded_params.set_log_dest(&log_config.log_dest, Some(stream))?;
                }
            } else {
                guarded_params.set_log_dest(&log_config.log_dest, NO_STREAM)?;
            }
        }

        Ok(())
    }

    // TODO: initialize from string loglevel instead
    pub fn initialise(level: Option<&str>) -> Result<(), LogError> {
        let logger = Logger::new();

        let mut log_level = DEFAULT_LOG_LEVEL;
        let level_set = if let Some(level) = level {
            log_level = Level::from_str(level).context(LogErrCtx::from_remark(
                LogErrorKind::InvParam,
                &format!("failed to parse LogLevel from '{}'", log_level),
            ))?;
            true
        } else {
            false
        };

        let log_config = if let Ok(config_path) = env::var("LOG_CONFIG") {
            Some(LogConfig::from_file(config_path)?)
        } else {
            None
        };

        let max_level = {
            let mut guarded_params = logger.inner.lock().unwrap();
            
            if guarded_params.is_initialized() {
                return Err(LogError::from_remark(
                    LogErrorKind::InvState,
                    "The logger is already initialzed",
                ));
            }

            if let Some(log_config) = log_config {
            
                if let Some(ref default_level) = log_config.default_level {
                    guarded_params.set_default_level(default_level);
                }

                guarded_params.set_mod_config(&log_config.mod_level);

                if log_config.log_dest != DEFAULT_LOG_DEST {
                    if log_config.log_dest == LogDestination::STREAM {
                        if let Some(ref log_path) = log_config.log_stream {
                            let stream = BufWriter::new(File::open(log_path).context(
                                LogErrCtx::from_remark(
                                    LogErrorKind::Upstream,
                                    &format!("Failed to open log file: '{}'", log_path.display()),
                                ),
                            )?);
                            guarded_params.set_log_dest(&log_config.log_dest, Some(stream))?;
                        }
                    } else {
                        guarded_params.set_log_dest(&log_config.log_dest, NO_STREAM)?;
                    }
                }
            }

            if level_set {
                guarded_params.set_default_level(&log_level);
            }

            guarded_params.set_initialized();
            guarded_params.get_max_level().clone()
        };

        log::set_boxed_logger(Box::new(logger)).context(LogErrCtx::from_remark(
            LogErrorKind::Upstream,
            "Logger::initialise: failed to initialize logger",
        ))?;

        log::set_max_level(max_level.to_level_filter());

        Ok(())
    }

    /*
        // TODO: not my favorite solution but the corresponding level function is private
        fn level_from_usize(level: usize) -> Option<Level> {
            match level {
                0 => None,
                1 => Some(Level::Info),
                2 => Some(Level::Debug),
                _ => Some(Level::Trace),
            }
        }
    */
}

impl Log for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        let mut mod_name = String::from("undefined");
        if let Some(mod_path) = record.module_path() {
            if let Some(ref captures) = self.module_re.captures(mod_path) {
                mod_name = String::from(captures.get(1).unwrap().as_str());
            }
        }

        let curr_level = &record.metadata().level();
        let mut guarded_params = self.inner.lock().unwrap();
        let mut level = guarded_params.get_default_level();
        if let Some(mod_level) = guarded_params.get_mod_level(&mod_name) {
            level = mod_level;
        }

        if curr_level <= level {
            let output = format!(
                "{} {:<5} [{}] {}\n",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level().to_string(),
                &mod_name,
                record.args()
            );

            let output = match curr_level {
                Level::Error => output.red(),
                Level::Warn => output.yellow(),
                Level::Info => output.green(),
                Level::Debug => output.cyan(),
                Level::Trace => output.blue(),
            };

            let _res = match guarded_params.get_log_dest() {
                LogDestination::STDERR => stderr().write(output.as_bytes()),
                LogDestination::STDOUT => stdout().write(output.as_bytes()),
                LogDestination::STREAM => {
                    if let Some(ref mut stream) = guarded_params.get_log_stream() {
                        stream.write(output.as_bytes())
                    } else {
                        stderr().write(output.as_bytes())
                    }
                }
            };
        }
    }

    fn flush(&self) {
        let mut guarded_params = self.inner.lock().unwrap();
        match guarded_params.get_log_dest() {
            LogDestination::STREAM => {
                if let Some(ref mut stream) = guarded_params.get_log_stream() {
                    let _res = stream.flush();
                }
            }
            LogDestination::STDERR => {
                let _res = stderr().flush();
            }
            LogDestination::STDOUT => {
                let _res = stdout().flush();
            }
        }
    }
}

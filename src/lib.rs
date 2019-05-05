use chrono::Local;
use colored::*;
use failure::ResultExt;
use log::{Level, Log, Metadata, Record};
use regex::Regex;
use std::collections::HashMap;
use std::env;
use std::io::{stderr, stdout, Write};
use std::mem;
use std::str::FromStr;
use std::sync::{Arc, Mutex, Once, ONCE_INIT}; //, BufWriter};
mod log_error;
use log_error::{LogErrCtx, LogError, LogErrorKind};

mod config;
use config::LogConfig;

pub(crate) const DEFAULT_LOG_LEVEL: Level = Level::Warn;

#[derive(Debug, Clone)]
pub enum LogDestination {
    STDOUT,
    STDERR,
    STREAM,
}

// cannot be STREAM !!
pub(crate) const DEFAULT_LOG_DEST: LogDestination = LogDestination::STDERR;

struct LoggerParams {
    log_dest: LogDestination,
    log_stream: Option<Box<Write + Send>>,
    initialized: bool,
    default_level: Level,
    mod_level: HashMap<String, Level>,
    max_level: Level,
}

impl LoggerParams {
    pub fn new(log_level: Level) -> LoggerParams {
        LoggerParams {
            log_dest: DEFAULT_LOG_DEST,
            log_stream: None,
            initialized: false,
            default_level: log_level,
            max_level: log_level,
            mod_level: HashMap::new(),
        }
    }

    pub fn set_max_level(&mut self) {
        // TODO: implement
        let mut max_level = self.default_level;
        for level in self.mod_level.values() {
            if &max_level < level {
                max_level = level.clone();
            }
        }
        self.max_level = max_level;
    }

    fn set_log_config(&mut self, log_config: &LogConfig) -> Level {
        let mut max_level = Level::Error;
        for module in log_config.mod_level.keys() {
            if let Some(ref level) = log_config.mod_level.get(module) {
                self.mod_level.insert(module.clone(), (*level).clone());

                if *level > &max_level {
                    max_level = (*level).clone();
                }
            }
        }

        // only set this if none was given in function parameters
        if let Some(default_level) = log_config.default_level {
            self.default_level = default_level;
        }

        max_level
    }
}

#[derive(Clone)]
pub struct Logger {
    inner: Arc<Mutex<LoggerParams>>,
    module_re: Regex,
}

impl Logger {
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

    pub fn set_log_level(log_level: Level) {
        let logger = Logger::new();
        let mut guarded_params = logger.inner.lock().unwrap();
        guarded_params.default_level = log_level;
        guarded_params.set_max_level();
    }

    pub fn set_mod_level(module: &str, log_level: Level) {
        let logger = Logger::new();
        let mut guarded_params = logger.inner.lock().unwrap();
        guarded_params
            .mod_level
            .insert(String::from(module), log_level);
        guarded_params.set_max_level();
    }

    pub fn set_log_dest<S: 'static + Write + Send>(
        dest: &LogDestination,
        stream: Option<S>,
    ) -> Result<(), LogError> {
        let logger = Logger::new();
        let mut guarded_params = logger.inner.lock().unwrap();

        match dest {
            LogDestination::STREAM => {
                if let Some(stream) = stream {
                    guarded_params.log_dest = dest.clone();
                    guarded_params.log_stream = Some(Box::new(stream));
                    Ok(())
                } else {
                    Err(LogError::from_remark(
                        LogErrorKind::InvParam,
                        &format!("no stream given for log destination type STREAM"),
                    ))
                }
            }
            _ => {
                guarded_params.log_stream = None;
                guarded_params.log_dest = dest.clone();
                Ok(())
            }
        }
    }

    pub fn set_mod_config(log_config: &LogConfig) {
        let logger = Logger::new();
        let mut guarded_params = logger.inner.lock().unwrap();
        let max_level = guarded_params.set_log_config(log_config);
        if max_level > guarded_params.max_level {
            guarded_params.max_level = max_level;
        }
    }

    pub fn get_log_level(&self) -> Level {
        let guarded_params = self.inner.lock().unwrap();
        guarded_params.default_level.clone()
    }

    // TODO: initialize from string loglevel instead
    pub fn initialise(level: Option<&str>) -> Result<(), LogError> {
        let logger = Logger::new();

        let mut log_level = DEFAULT_LOG_LEVEL;
        let mut level_set = false;

        if let Some(level) = level {
            log_level = Level::from_str(level).context(LogErrCtx::from_remark(
                LogErrorKind::InvParam,
                &format!("failed to parse LogLevel from '{}'", log_level),
            ))?;
            level_set = true;
        }

        let log_config = if let Ok(config_path) = env::var("LOG_CONFIG") {
            Some(LogConfig::from_file(config_path)?)
        } else {
            None
        };

        let (initialized, max_level, last_max_level) = {
            let mut guarded_params = logger.inner.lock().unwrap();
            let initialized = guarded_params.initialized;
            let last_max_level = guarded_params.max_level;

            let mut max_level = if let Some(log_config) = log_config {
                guarded_params.set_log_config(&log_config)
            } else {
                Level::max()
            };

            if level_set {
                guarded_params.default_level = log_level;
            }

            if guarded_params.default_level > max_level {
                max_level = guarded_params.default_level;
            }

            guarded_params.max_level = max_level;
            guarded_params.initialized = true;

            (initialized, max_level, last_max_level)
        };

        if initialized == false {
            log::set_boxed_logger(Box::new(logger)).context(LogErrCtx::from_remark(
                LogErrorKind::Upstream,
                "Logger::initialise: failed to initialize logger",
            ))?;
        }

        if last_max_level != max_level {
            log::set_max_level(max_level.to_level_filter());
        }

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

        let curr_level = record.metadata().level();
        let mut guarded_params = self.inner.lock().unwrap();
        let mut level = guarded_params.default_level;
        if let Some(mod_level) = guarded_params.mod_level.get(&mod_name) {
            level = *mod_level;
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

            let _res = match guarded_params.log_dest {
                LogDestination::STDERR => stderr().write(output.as_bytes()),
                LogDestination::STDOUT => stdout().write(output.as_bytes()),
                LogDestination::STREAM => {
                    if let Some(ref mut stream) = guarded_params.log_stream {
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
        match guarded_params.log_dest {
            LogDestination::STREAM => {
                if let Some(ref mut stream) = guarded_params.log_stream {
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

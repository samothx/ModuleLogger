use chrono::Local;
use colored::*;
use failure::ResultExt;
use log::{Log, Metadata, Record};
use regex::Regex;
use std::env;
use std::fs::OpenOptions;
use std::io::{stderr, stdout, BufWriter, Write};
use std::mem;
use std::str::FromStr;
use std::sync::{Arc, Mutex, Once, ONCE_INIT}; //, BufWriter};
mod log_error;
use log_error::{LogErrCtx, LogError, LogErrorKind};

mod config;
use config::LogConfig;

mod logger_params;
pub use logger_params::LogDestination;
use logger_params::LoggerParams;

pub(crate) const DEFAULT_LOG_LEVEL: Level = Level::Warn;

// cannot be STREAM !!
pub(crate) const DEFAULT_LOG_DEST: LogDestination = LogDestination::Stderr;

pub const NO_STREAM: Option<Box<'static + Write + Send>> = None;

pub use log::Level;

// TODO: implement size limit for memory buffer

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

    pub fn flush() {
        Logger::new().flush();
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

    pub fn get_buffer() -> Option<Vec<u8>> {
        let logger = Logger::new();
        let mut guarded_params = logger.inner.lock().unwrap();
        guarded_params.retrieve_log_buffer()
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

    pub fn get_log_dest() -> LogDestination {
        let logger = Logger::new();
        let mut guarded_params = logger.inner.lock().unwrap();
        guarded_params.get_log_dest().clone()
    }


    pub fn set_log_config(log_config: &LogConfig) -> Result<(), LogError> {
        let logger = Logger::new();
        let mut guarded_params = logger.inner.lock().unwrap();
        let last_max_level = guarded_params.get_max_level().clone();

        guarded_params.set_default_level(&log_config.get_default_level()?);

        let max_level = guarded_params.set_mod_config(&log_config.get_mod_level()?);
        if max_level != &last_max_level {
            log::set_max_level(max_level.to_level_filter());
        }

        let log_dest = guarded_params.get_log_dest();
        let cfg_log_dest = log_config.get_log_dest();
        let stream_log = log_dest.is_stream_dest();

        if cfg_log_dest != log_dest || stream_log == true {
            if stream_log == true {
                if let Some(log_path) = log_config.get_log_stream() {
                    let stream = BufWriter::new(
                        OpenOptions::new()
                            .write(true)
                            .append(true)
                            .create(true)
                            .open(log_path)
                            .context(LogErrCtx::from_remark(
                                LogErrorKind::Upstream,
                                &format!("Failed to open log file: '{}'", log_path.display()),
                            ))?,
                    );
                    guarded_params.set_log_dest(cfg_log_dest, Some(stream))?;
                }
            } else {
                guarded_params.set_log_dest(cfg_log_dest, NO_STREAM)?;
            }
        }

        Ok(())
    }

    // TODO: initialize from string loglevel instead
    pub fn initialise(level: Option<&str>) -> Result<(), LogError> {
        let logger = Logger::new();

        let (log_level, level_set) = if let Some(level) = level {
            (
                Level::from_str(level).context(LogErrCtx::from_remark(
                    LogErrorKind::InvParam,
                    &format!("failed to parse LogLevel from '{}'", level),
                ))?,
                true,
            )
        } else {
            (DEFAULT_LOG_LEVEL, false)
        };

        let max_level = {
            let mut guarded_params = logger.inner.lock().unwrap();

            if guarded_params.is_initialized() {
                return Err(LogError::from_remark(
                    LogErrorKind::InvState,
                    "The logger is already initialzed",
                ));
            }

            if let Ok(config_path) = env::var("LOG_CONFIG") {
                let log_config = LogConfig::from_file(config_path)?;

                guarded_params.set_default_level(&log_config.get_default_level()?);

                guarded_params.set_mod_config(&log_config.get_mod_level()?);

                let cfg_log_dest = log_config.get_log_dest();

                if cfg_log_dest != &DEFAULT_LOG_DEST {
                    if cfg_log_dest.is_stream_dest() {
                        if let Some(log_path) = log_config.get_log_stream() {
                            let stream = BufWriter::new(
                                OpenOptions::new()
                                    .write(true)
                                    .append(true)
                                    .create(true)
                                    .open(log_path)
                                    .context(LogErrCtx::from_remark(
                                        LogErrorKind::Upstream,
                                        &format!(
                                            "Failed to open log file: '{}'",
                                            log_path.display()
                                        ),
                                    ))?,
                            );
                            guarded_params.set_log_dest(cfg_log_dest, Some(stream))?;
                        }
                    } else {
                        guarded_params.set_log_dest(cfg_log_dest, NO_STREAM)?;
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

        let curr_level = &record.metadata().level();
        let mut guarded_params = self.inner.lock().unwrap();
        let mut level = guarded_params.get_default_level();
        if let Some(mod_level) = guarded_params.get_mod_level(&mod_tag) {
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
                Level::Error => format!("{}", output.red()),
                Level::Warn => format!("{}", output.yellow()),
                Level::Info => format!("{}", output.green()),
                Level::Debug => format!("{}", output.cyan()),
                Level::Trace => format!("{}", output.blue()),
            };

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
            ()
        }
    }

    fn flush(&self) {
        let mut guarded_params = self.inner.lock().unwrap();
        match guarded_params.get_log_dest() {
            LogDestination::Stream => {
                if let Some(ref mut stream) = guarded_params.get_log_stream() {
                    let _res = stream.flush();
                }
            }
            LogDestination::StreamStderr => {
                if let Some(ref mut stream) = guarded_params.get_log_stream() {
                    let _res = stream.flush();
                    let _res = stderr().flush();
                }
            }
            LogDestination::StreamStdout => {
                if let Some(ref mut stream) = guarded_params.get_log_stream() {
                    let _res = stream.flush();
                    let _res = stdout().flush();
                }
            }
            LogDestination::Buffer => {}
            LogDestination::BufferStderr => {
                let _res = stderr().flush();
            }
            LogDestination::BufferStdout => {
                let _res = stdout().flush();
            }
            LogDestination::Stderr => {
                let _res = stderr().flush();
            }
            LogDestination::Stdout => {
                let _res = stdout().flush();
            }
        }
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

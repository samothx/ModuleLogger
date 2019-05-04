use chrono::Local;
use colored::*;
use failure::ResultExt;
use log::{Level, Log, Metadata, Record};
use regex::Regex;
use std::collections::HashMap;
use std::env;

mod log_error;
use log_error::{LogError, LogErrCtx, LogErrorKind};

mod config;
use config::LogConfig;

const MODULE: &str = "common::logger";

pub const DEFAULT_LOG_LEVEL: Level = Level::Warn;

#[derive(Debug)]
pub struct Logger {
    default_level: Level,
    mod_level: HashMap<String, Level>,
    module_re: Regex,
}

impl Logger {
    pub fn initialise(default_log_level: usize) -> Result<(), LogError> {
        // config:  &Option<LogConfig>)

        let mut logger = Logger {
            default_level: DEFAULT_LOG_LEVEL,
            mod_level: HashMap::new(),
            module_re: Regex::new(r#"^[^:]+::(.*)$"#).unwrap(),
        };

        let mut max_level = logger.default_level;

        if let Ok(config_path) = env::var("LOG_CONFIG") {
            LogConfig::from_file(config_path)?;
        }

        if let Some(level) = Logger::level_from_usize(default_log_level) {
            logger.default_level = level;
        }

        if logger.default_level > max_level {
            max_level = logger.default_level;
        }

        log::set_boxed_logger(Box::new(logger)).context(LogErrCtx::from_remark(
            LogErrorKind::Upstream,
            &format!("{}::initialise: failed to initialize logger", MODULE),
        ))?;
        log::set_max_level(max_level.to_level_filter());

        Ok(())
    }

    // TODO: not my favorite solution but the corresponding level function is private
    fn level_from_usize(level: usize) -> Option<Level> {
        match level {
            0 => None,
            1 => Some(Level::Info),
            2 => Some(Level::Debug),
            _ => Some(Level::Trace),
        }
    }
}

impl Log for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        let mut level = self.default_level;

        let mut mod_name = String::from("undefined");
        if let Some(mod_path) = record.module_path() {
            if let Some(ref captures) = self.module_re.captures(mod_path) {
                mod_name = String::from(captures.get(1).unwrap().as_str());
            }
        }

        if let Some(mod_level) = self.mod_level.get(&mod_name) {
            level = *mod_level;
        }

        let curr_level = record.metadata().level();
        if curr_level <= level {
            let output = format!(
                "{} {:<5} [{}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level().to_string(),
                &mod_name,
                record.args()
            );

            match curr_level {
                Level::Error => println!("{}", output.red()),
                Level::Warn => println!("{}", output.yellow()),
                Level::Info => println!("{}", output.green()),
                Level::Debug => println!("{}", output.cyan()),
                Level::Trace => println!("{}", output.blue()),
            };
        }
    }

    fn flush(&self) {}
}

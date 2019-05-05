use chrono::Local;
use colored::*;
use failure::ResultExt;
use log::{Level, Log, Metadata, Record};
use regex::Regex;
use std::collections::HashMap;
use std::env;
use std::sync::{Arc, Mutex, Once, ONCE_INIT};
use std::{mem};
use std::str::FromStr;


mod log_error;
use log_error::{LogError, LogErrCtx, LogErrorKind};

mod config;
use config::LogConfig;

pub const DEFAULT_LOG_LEVEL: Level = Level::Warn;


#[derive(Debug)]
struct LoggerParams {
    initialized: bool,
    default_level: Level,
    mod_level: HashMap<String, Level>,
    max_level: Level,
}

impl LoggerParams {
    pub fn new(log_level: Level) -> LoggerParams {
        LoggerParams{
            initialized: false,
            default_level: log_level, 
            max_level: log_level,
            mod_level: HashMap::new(),
        }
    }

    pub fn set_max_level(&mut self) {
        // TODO: implement
    }

}

#[derive(Debug,Clone)]
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

    pub fn set_log_level(&mut self, log_level: Level) {
        let mut guarded_params = self.inner.lock().unwrap();
        guarded_params.default_level = log_level;
        guarded_params.set_max_level();
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
        let mut initialized = false;
        let mut max_level = Level::Error;
        let mut last_max_level = max_level;
        

        if let Some(level) = level {
            log_level = Level::from_str(level).context(LogErrCtx::from_remark(LogErrorKind::InvParam,&format!("failed to parse LogLevel from '{}'", log_level)))?;
            level_set = true;
        }

        let log_config = 
            if let Ok(config_path) = env::var("LOG_CONFIG") {
                Some(LogConfig::from_file(config_path)?)
            } else {
                None
            };

        
            {            
            let mut guarded_params = logger.inner.lock().unwrap();
            initialized = guarded_params.initialized;
            last_max_level = guarded_params.max_level;
            
            if let Some(log_config) = log_config {
                for (module,level) in log_config.mod_level {
                    guarded_params.mod_level.insert(String::from(module), level);
                    if level > max_level {
                        max_level = level;
                    }                    
                }

                // only set this if none was given in function parameters
                if !level_set {
                    if let Some(default_level) = log_config.default_level {
                        guarded_params.default_level = default_level;
                    }
                }
            }

            if level_set {           
                guarded_params.default_level = log_level;
            }

            if guarded_params.default_level > max_level {
                max_level = guarded_params.default_level;
            }    

            guarded_params.max_level = max_level;                            
            guarded_params.initialized = true;
            }

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

        let level = {            
            let guarded_params = self.inner.lock().unwrap();
            let mut level = guarded_params.default_level;
            if let Some(mod_level) = guarded_params.mod_level.get(&mod_name) {
                level = *mod_level;
            }
            level
        };

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

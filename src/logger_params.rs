use log::{Level};
use std::collections::HashMap;
use std::io::{Write};

use super::{
    DEFAULT_LOG_DEST,
    LogError,
    LogErrorKind,
};


#[derive(Debug, Clone, PartialEq)]
pub enum LogDestination {
    STDOUT,
    STDERR,
    STREAM,
}

pub(crate) struct LoggerParams {
    log_dest: LogDestination,
    log_stream: Option<Box<Write + Send>>,
    initialized: bool,
    default_level: Level,
    mod_level: HashMap<String, Level>,
    max_level: Level,
}

impl<'a> LoggerParams {
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

    pub fn is_initialized(&mut self) -> bool {
        self.initialized
    }

    pub fn set_initialized(&mut self) -> bool {
        if ! self.initialized {
            self.initialized = true;
            false 
        } else {
            true
        }        
    } 

    fn recalculate_max_level(&mut self) {
        // TODO: implement
        let mut max_level = self.default_level;
        for level in self.mod_level.values() {
            if &max_level < level {
                max_level = level.clone();
            }
        }
        self.max_level = max_level;
    }

    pub fn get_max_level(&'a self) -> &'a Level {
        &self.max_level
    }

    pub fn get_mod_level(&'a self, module: &str) -> Option<&'a Level> {
        if let Some(ref level) = self.mod_level.get(module) {
            Some(level)
        } else {
            None
        }
    }

    pub fn set_mod_level(&'a mut self, module: &str, level: &Level) -> &'a Level {        
        self.mod_level.insert(String::from(module), level.clone());
        if level > &self.max_level {
            self.max_level = level.clone();
        } else if level < &self.max_level {
            self.recalculate_max_level();
        }
        &self.max_level
    }

    pub fn set_mod_config(&'a mut self, mod_config: &HashMap<String,Level>) -> &'a Level {        
        for module in mod_config.keys() {
            if let Some(ref level) = mod_config.get(module) {
                self.mod_level.insert(module.clone(), (*level).clone());
            }
        }
        self.recalculate_max_level();
        &self.max_level
    }

    pub fn set_default_level(&'a mut self, level: &Level) -> &'a Level {        
        self.default_level = level.clone();
        if level > &self.max_level {
            self.max_level = level.clone();
        } else if level < &self.max_level {
            self.recalculate_max_level()
        }
        &self.max_level
    }

    pub fn get_default_level(&'a self) -> &'a Level {
        &self.default_level
    }

    pub fn get_log_dest(&'a self) -> &'a LogDestination {
        &self.log_dest
    }

    pub fn get_log_stream(&mut self) -> &mut Option<Box<'static +Write + Send>> {
        &mut self.log_stream
    }

    pub fn set_log_dest<S:'static +Write + Send>(&mut self, dest: &LogDestination, stream: Option<S>) -> Result<(),LogError> {
        // TODO: flush ? 
        match dest {
            LogDestination::STREAM => {
                if let Some(stream) = stream {
                    self.log_dest = dest.clone();
                    self.log_stream = Some(Box::new(stream));
                    Ok(())
                } else {
                    Err(LogError::from_remark(
                        LogErrorKind::InvParam,
                        "no stream given for log destination type STREAM",
                    ))
                }
            },
            _ => {
                self.log_stream = None;
                self.log_dest = dest.clone();
                Ok(())
            }
        }
    }
}

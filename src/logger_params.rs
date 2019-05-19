use log::Level;
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Write;

use super::{LogError, LogErrorKind, DEFAULT_LOG_DEST};

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub enum LogDestination {
    Stdout,
    Stderr,
    Stream,
    StreamStdout,
    StreamStderr,
    Buffer,
    BufferStdout,
    BufferStderr,
}

impl LogDestination {
    pub fn is_stream_dest(&self) -> bool {
        self == &LogDestination::Stream
            || self == &LogDestination::StreamStderr
            || self == &LogDestination::StreamStdout
    }

    pub fn is_buffer_dest(&self) -> bool {
        self == &LogDestination::Buffer
            || self == &LogDestination::BufferStderr
            || self == &LogDestination::BufferStdout
    }
}

pub(crate) struct LoggerParams {
    log_dest: LogDestination,
    log_stream: Option<Box<Write + Send>>,
    log_buffer: Option<Vec<u8>>,
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
            log_buffer: None,
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
        if !self.initialized {
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

    pub fn set_mod_config(&'a mut self, mod_config: &HashMap<String, Level>) -> &'a Level {
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

    pub fn get_log_stream(&mut self) -> &mut Option<Box<'static + Write + Send>> {
        &mut self.log_stream
    }

    pub fn get_log_buffer(&mut self) -> Option<&mut Vec<u8>> {
        if let Some(ref mut buffer) = self.log_buffer {
            Some(buffer)
        } else {
            None
        }
    }

    pub fn retrieve_log_buffer(&mut self) -> Option<Vec<u8>> {
        if let Some(ref mut buffer) = self.log_buffer {
            let tmp = buffer.clone();
            buffer.clear();
            Some(tmp)
        } else {
            None
        }
    }

    pub fn set_log_dest<S: 'static + Write + Send>(
        &mut self,
        dest: &LogDestination,
        stream: Option<S>,
    ) -> Result<(), LogError> {
        // TODO: flush ?

        if dest.is_stream_dest() {
            if let Some(stream) = stream {
                self.log_dest = dest.clone();
                self.log_stream = Some(Box::new(stream));
                Ok(())
            } else {
                Err(LogError::from_remark(
                    LogErrorKind::InvParam,
                    &format!("no stream given for log destination type {:?}", dest),
                ))
            }
        } else if dest.is_buffer_dest() {
            self.log_dest = dest.clone();
            self.log_stream = None;
            self.log_buffer = Some(Vec::new());
            Ok(())
        } else {
            self.log_stream = None;
            self.log_dest = dest.clone();
            Ok(())
        }
    }
}

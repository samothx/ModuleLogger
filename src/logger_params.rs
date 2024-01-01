use log::Level;
#[cfg(feature = "config")]
use serde::Deserialize;
use std::collections::HashMap;
use std::io::{stderr, stdout, Write};
use std::result;

use super::{Error, ErrorKind, Result, DEFAULT_LOG_DEST};
use std::cmp::Ordering;
use std::str::FromStr;

cfg_if::cfg_if! {
    if #[cfg(feature = "config")] {
        #[derive(Debug, Clone, PartialEq, Deserialize)]
        pub enum LogDestination {
            /// log to stdout
            Stdout,
            /// log to stderr
            Stderr,
            /// log to an output file
            Stream,
            /// log to an output file and to stdout
            StreamStdout,
            /// log to an output file and to stderr
            StreamStderr,
            /// log to a memory buffer
            Buffer,
            /// log to stdout and to a memory buffer
            BufferStdout,
            /// log to stderr and to a memory buffer
            BufferStderr,
        }
    } else {
        #[derive(Debug, Clone, PartialEq)]
        pub enum LogDestination {
            /// log to stdout
            Stdout,
            /// log to stderr
            Stderr,
            /// log to an output file
            Stream,
            /// log to an output file and to stdout
            StreamStdout,
            /// log to an output file and to stderr
            StreamStderr,
            /// log to a memory buffer
            Buffer,
            /// log to stdout and to a memory buffer
            BufferStdout,
            /// log to stderr and to a memory buffer
            BufferStderr,
        }
    }
}

const DEST_TX: &[(&str, LogDestination); 8] = &[
    ("stdout", LogDestination::Stdout),
    ("stderr", LogDestination::Stderr),
    ("stream", LogDestination::Stream),
    ("streamstdout", LogDestination::StreamStdout),
    ("streamstderr", LogDestination::StreamStderr),
    ("buffer", LogDestination::Buffer),
    ("bufferstdout", LogDestination::BufferStdout),
    ("bufferstderr", LogDestination::BufferStderr),
];

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

    pub fn is_stderr(&self) -> bool {
        self == &LogDestination::Stderr
            || self == &LogDestination::BufferStderr
            || self == &LogDestination::StreamStderr
    }

    pub fn is_stdout(&self) -> bool {
        self == &LogDestination::Stdout
            || self == &LogDestination::BufferStdout
            || self == &LogDestination::StreamStdout
    }
}

impl FromStr for LogDestination {
    type Err = Error;
    fn from_str(dest: &str) -> result::Result<Self, Self::Err> {
        if let Some(pos) = DEST_TX
            .iter()
            .position(|val| val.0.eq_ignore_ascii_case(dest))
        {
            Ok(DEST_TX[pos].1.clone())
        } else {
            Err(Error::with_context(
                ErrorKind::InvParam,
                &format!("Invalid log destination string encountered: '{}'", dest),
            ))
        }
    }
}

pub(crate) struct LoggerParams {
    log_dest: LogDestination,
    log_stream: Option<Box<dyn Write + Send>>,
    log_buffer: Option<Vec<u8>>,
    default_level: Level,
    mod_level: HashMap<String, Level>,
    max_level: Level,
    color: bool,
    brief_info: bool,
    timestamp: bool,
    millis: bool,
    initialised: bool,
}

impl<'a> LoggerParams {
    pub fn new(log_level: Level) -> LoggerParams {
        LoggerParams {
            log_dest: DEFAULT_LOG_DEST,
            log_stream: None,
            log_buffer: None,
            default_level: log_level,
            max_level: log_level,
            mod_level: HashMap::new(),
            initialised: false,
            color: false,
            brief_info: false,
            timestamp: true,
            millis: false,
        }
    }

    pub fn initialised(&mut self) -> bool {
        if self.initialised {
            true
        } else {
            self.initialised = true;
            false
        }
    }

    fn recalculate_max_level(&mut self) {
        // TODO: implement
        let mut max_level = self.default_level;
        for level in self.mod_level.values() {
            if max_level < *level {
                max_level = *level;
            }
        }
        self.max_level = max_level;
    }

    pub fn max_level(&'a self) -> &'a Level {
        &self.max_level
    }

    pub fn get_mod_level(&'a self, module: &str) -> Option<Level> {
        let mut mod_path = module;

        loop {
            if let Some(level) = self.mod_level.get(mod_path) {
                return Some(*level);
            }
            if let Some(index) = mod_path.rfind("::") {
                let (mod_new, _dumm) = mod_path.split_at(index);
                mod_path = mod_new;
            } else {
                return None;
            }
        }
    }

    pub fn set_color(&'a mut self, color: bool) {
        self.color = color;
    }

    pub fn color(&'a mut self) -> bool {
        self.color
    }

    pub fn set_brief_info(&'a mut self, val: bool) {
        self.brief_info = val;
    }
    pub fn brief_info(&'a mut self) -> bool {
        self.brief_info
    }

    pub fn set_timestamp(&'a mut self, val: bool) {
        self.timestamp = val;
    }
    pub fn timestamp(&'a mut self) -> bool {
        self.timestamp
    }

    pub fn set_millis(&'a mut self, val: bool) {
        self.millis = val;
    }
    pub fn millis(&'a mut self) -> bool {
        self.millis
    }

    pub fn set_mod_level(&'a mut self, module: &str, level: Level) -> &'a Level {
        self.mod_level.insert(String::from(module), level);
        match level.cmp(&self.max_level) {
            Ordering::Greater => {
                self.max_level = level;
            }
            Ordering::Less => {
                self.recalculate_max_level();
            }
            _ => (),
        };
        &self.max_level
    }

    #[cfg(feature = "config")]
    pub fn set_mod_config(&'a mut self, mod_config: &HashMap<String, Level>) -> &'a Level {
        for module in mod_config.keys() {
            if let Some(level) = mod_config.get(module) {
                self.mod_level.insert(module.clone(), *level);
            }
        }
        self.recalculate_max_level();
        &self.max_level
    }

    pub fn set_default_level(&'a mut self, level: Level) -> Level {
        self.default_level = level;
        if level >= self.max_level {
            self.max_level = level;
        } else {
            self.recalculate_max_level()
        }
        self.max_level
    }

    pub fn get_default_level(&'a self) -> Level {
        self.default_level
    }

    pub fn get_log_dest(&'a self) -> &'a LogDestination {
        &self.log_dest
    }

    pub fn log_stream(&mut self) -> &mut Option<Box<dyn 'static + Write + Send>> {
        &mut self.log_stream
    }

    pub fn log_buffer(&mut self) -> Option<&mut Vec<u8>> {
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

    pub fn flush(&mut self) {
        if self.log_dest.is_stream_dest() {
            if let Some(ref mut stream) = self.log_stream() {
                let _res = stream.flush();
            }
        }

        if self.log_dest.is_stderr() {
            let _res = stderr().flush();
        } else if self.log_dest.is_stdout() {
            let _res = stdout().flush();
        }
    }

    pub fn set_log_dest<S: 'static + Write + Send>(
        &mut self,
        dest: &LogDestination,
        stream: Option<S>,
    ) -> Result<()> {
        // TODO: flush ?

        self.flush();

        if dest.is_stream_dest() {
            if let Some(stream) = stream {
                self.log_dest = dest.clone();
                self.log_stream = Some(Box::new(stream));
                Ok(())
            } else {
                Err(Error::with_context(
                    ErrorKind::InvParam,
                    &format!("no stream given for log destination type {:?}", dest),
                ))
            }
        } else if dest.is_buffer_dest() {
            self.log_dest = dest.clone();
            self.log_stream = None;
            if self.log_buffer.is_none() {
                self.log_buffer = Some(Vec::new());
            }
            Ok(())
        } else {
            self.log_stream = None;
            self.log_dest = dest.clone();
            if self.log_buffer.is_some() {
                self.log_buffer = None;
            }
            Ok(())
        }
    }
}

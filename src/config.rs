#![cfg(feature = "config")]
use log::{trace, Level};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde_yaml;

use crate::{
    error::{Error, ErrorKind, Result, ToError},
    LogDestination, DEFAULT_LOG_DEST, DEFAULT_LOG_LEVEL,
};

// TODO: create log config builder and initialise Logger with config object, instead of using complex parameters for Logger::initialise

#[derive(Debug, Deserialize)]
struct LogConfigFile {
    default_level: Option<String>,
    mod_level: Option<HashMap<String, String>>,
    log_dest: Option<String>,
    log_stream: Option<PathBuf>,
    color: Option<bool>,
    brief_info: Option<bool>,
    // TODO: allow to configure buffer max, implement ring buffer for log
}

pub struct LogConfig {
    default_level: Level,
    mod_level: HashMap<String, Level>,
    log_dest: LogDestination,
    log_stream: Option<PathBuf>,
    color: bool,
    brief_info: bool,
}

/// The logger configuration parameters
/// Used in Logger::set_log_config
impl<'a> LogConfig {
    pub(crate) fn get_default_level(&'a self) -> Level {
        self.default_level
    }

    pub(crate) fn get_mod_level(&'a self) -> &'a HashMap<String, Level> {
        &self.mod_level
    }

    pub(crate) fn get_log_dest(&'a self) -> &'a LogDestination {
        &self.log_dest
    }

    pub(crate) fn get_log_stream(&'a self) -> &'a Option<PathBuf> {
        &self.log_stream
    }

    pub(crate) fn is_color(&self) -> bool {
        self.color
    }

    pub(crate) fn is_brief_info(&self) -> bool {
        self.brief_info
    }
}

pub struct LogConfigBuilder {
    inner: LogConfig,
}

/// LogConfigBuilder helps creating a configuration for logger.
impl<'a> LogConfigBuilder {
    /// Create a new LogConfigBuilder with defaults for all config settings.
    pub fn new() -> LogConfigBuilder {
        LogConfigBuilder {
            inner: LogConfig {
                default_level: DEFAULT_LOG_LEVEL,
                mod_level: HashMap::new(),
                log_dest: DEFAULT_LOG_DEST,
                log_stream: None,
                color: false,
                brief_info: false,
            },
        }
    }

    /// Create LogConfigBuilder with initial values taken from a YAML config file and defaults
    pub fn from_file<P: AsRef<Path>>(filename: P) -> Result<LogConfigBuilder> {
        trace!("from_file: entered");
        let config_path = filename.as_ref();

        let config_str = &read_to_string(&config_path).upstream_with_context(&format!(
            "config::from_file: failed to read {}",
            config_path.display()
        ))?;

        let cfg_file: LogConfigFile = serde_yaml::from_str(config_str)
            .upstream_with_context("failed to deserialze config from yaml")?;

        let mut builder = LogConfigBuilder::new();

        if let Some(ref level_str) = cfg_file.default_level {
            builder.inner.default_level = Level::from_str(level_str)
                .upstream_with_context(&format!("Invalid log level: '{}'", level_str))?;
        }

        if let Some(ref mod_level) = cfg_file.mod_level {
            for (mod_name, mod_level) in mod_level {
                builder.inner.mod_level.insert(
                    mod_name.clone(),
                    Level::from_str(mod_level).error_with_all(
                        ErrorKind::InvParam,
                        &format!("Invalid log level: '{}'", mod_level),
                    )?,
                );
            }
        }

        if let Some(ref dest_str) = cfg_file.log_dest {
            let dest = LogDestination::from_str(dest_str)?;
            if dest.is_stream_dest() {
                if let Some(stream) = cfg_file.log_stream {
                    builder.inner.log_stream = Some(stream)
                } else {
                    return Err(Error::with_context(
                        ErrorKind::InvParam,
                        &format!(
                            "Missing log stream parameter for log destination {:?}",
                            dest
                        ),
                    ));
                }
            }
            // TODO: read params for future ring buffer size
        }

        if let Some(color) = cfg_file.color {
            builder.inner.color = color;
        }

        if let Some(brief_info) = cfg_file.brief_info {
            builder.inner.brief_info = brief_info;
        }

        Ok(builder)
    }

    /// Set the default log Level
    pub fn set_default_level(&'a mut self, level: Level) -> &'a mut LogConfigBuilder {
        self.inner.default_level = level;
        self
    }

    /// Set the log level for a module
    /// Format of module is <module>[::<submodule>[::<submodule>]]
    pub fn set_mod_level(&'a mut self, module: &str, level: Level) -> &'a mut LogConfigBuilder {
        let _dummy = self.inner.mod_level.insert(String::from(module), level);
        self
    }

    /// Set log destination
    /// For stream type destinations the file must be supplied
    pub fn set_log_dest(
        &'a mut self,
        dest: LogDestination,
        file: Option<&PathBuf>,
    ) -> Result<&'a mut LogConfigBuilder> {
        if dest.is_stream_dest() {
            if let Some(stream) = file {
                self.inner.log_stream = Some(stream.clone());
            } else {
                return Err(Error::with_context(
                    ErrorKind::InvParam,
                    &format!("Missing parameter stream for log destination: {:?}", dest),
                ));
            }
        }
        self.inner.log_dest = dest;

        Ok(self)
    }

    /// Enable / disable brief info format.
    /// Brief info displays info messages without the source module
    pub fn set_brief_info(&'a mut self, val: bool) {
        self.inner.brief_info = val;
    }

    /// Enable / disable colored output
    pub fn set_color(&'a mut self, val: bool) {
        self.inner.color = val;
    }

    /// Build the configuration
    pub fn build(&'a self) -> &'a LogConfig {
        &self.inner
    }

    // TODO: implement setters for all parameters
}

impl Default for LogConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

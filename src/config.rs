use failure::ResultExt;
use log::{trace, Level};
use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};
use std::str::FromStr;
/*use std::fmt;

use serde::{
    de::{self, Unexpected},
    Deserialize, Deserializer,
};
*/
use serde::Deserialize;

use serde_yaml;

use crate::{
    log_error::{LogErrCtx, LogError, LogErrorKind},
    LogDestination, DEFAULT_LOG_DEST, DEFAULT_LOG_LEVEL,
};

// TODO: create log config builder and initialise Logger with config object, instead of using complex parameters for Logger::initialise


/* 
// TODO: try to automatically deserialze Levels - problem is wrapped in Option / Hashmap
struct DeserializeLevelVisitor;

impl<'de> de::Visitor<'de> for DeserializeLevelVisitor {
    type Value = Level;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a string")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match Level::from_str(v) {
            Ok(val) => Ok(val),
            Err(_why) => Err(E::invalid_value(Unexpected::Str(v), &self)),
        }
    }
}

fn deserialize_level<'de, D>(deserializer: D) -> Result<Level, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_any(DeserializeLevelVisitor)
}
*/


#[derive(Debug, Deserialize)]
struct LogConfigFile {
    default_level: Option<String>,
    mod_level: Option<HashMap<String, String>>,
    log_dest: Option<String>,
    log_stream: Option<PathBuf>,
    color: Option<bool>,
    no_mod: Option<bool>,
    // TODO: allow to configure buffer max, implement ring buffer for log
}

pub struct LogConfig {
    default_level: Level,
    mod_level: HashMap<String, Level>,
    log_dest: LogDestination,
    log_stream: Option<PathBuf>,
    color: bool,
    no_mod: bool,
}

impl<'a> LogConfig {
    pub fn builder() -> LogConfigBuilder {
        LogConfigBuilder::new()
    }

    #[doc = "Get Default Log Level"]
    pub fn get_default_level(&'a self) -> &'a Level {
        &self.default_level
    }

    #[doc = "Get Module Log Levels"]
    pub fn get_mod_level(&'a self) -> &'a HashMap<String, Level> {
        &self.mod_level
    }

    #[doc = "Get Log Destination"]
    pub fn get_log_dest(&'a self) -> &'a LogDestination {
        &self.log_dest
    }

    #[doc = "Get Log Stream for stream type Log Destinations"]
    pub fn get_log_stream(&'a self) -> &'a Option<PathBuf> {
        &self.log_stream
    }

    pub fn is_color(&self) -> bool {
        self.color
    }

    pub fn is_no_mod(&self) -> bool { self.color }
}

pub struct LogConfigBuilder {
    inner: LogConfig,
}

impl<'a> LogConfigBuilder {
    pub fn new() -> LogConfigBuilder {
        LogConfigBuilder {
            inner: LogConfig {
                default_level: DEFAULT_LOG_LEVEL,
                mod_level: HashMap::new(),
                log_dest: DEFAULT_LOG_DEST,
                log_stream: None,
                color: false,
                no_mod: false,
            },
        }
    }

    #[doc = "Create LogConfig from file"]
    pub fn from_file<P: AsRef<Path>>(
        filename: P,
    ) -> Result<LogConfigBuilder, LogError> {
        trace!("from_file: entered");
        let config_path = filename.as_ref();

        let config_str = &read_to_string(&config_path).context(LogErrCtx::from_remark(
            LogErrorKind::Upstream,
            &format!(
                "config::from_file: failed to read {}",
                config_path.display()
            ),
        ))?;

        let cfg_file: LogConfigFile =
            serde_yaml::from_str(config_str).context(LogErrCtx::from_remark(
                LogErrorKind::Upstream,
                "failed to deserialze config from yaml",
            ))?;

        let mut builder = LogConfigBuilder::new();

        if let Some(ref level_str) = cfg_file.default_level {
            builder.inner.default_level =
                Level::from_str(level_str).context(LogErrCtx::from_remark(
                    LogErrorKind::InvParam,
                    &format!("Invalid log level: '{}'", level_str),
                ))?;
        }

        if let Some(ref mod_level) = cfg_file.mod_level {
            for (mod_name, mod_level) in mod_level {
                builder.inner.mod_level.insert(
                    mod_name.clone(),
                    Level::from_str(mod_level).context(LogErrCtx::from_remark(
                        LogErrorKind::InvParam,
                        &format!("Invalid log level: '{}'", mod_level),
                    ))?,
                );
            }
        }

        if let Some(ref dest_str) = cfg_file.log_dest {
            let dest = LogDestination::from_str(dest_str)?;
            if dest.is_stream_dest() {
                if let Some(stream) = cfg_file.log_stream {
                    builder.inner.log_stream = Some(stream)
                } else {
                    return Err(LogError::from_remark(
                        LogErrorKind::InvParam,
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

        println!("from_file: checking no_mod {:?}", cfg_file.no_mod);
        if let Some(no_mod) = cfg_file.no_mod {
            println!("from_file: setting no_mod to {}", no_mod);
            builder.inner.no_mod = no_mod;
        }

        Ok(builder)
    }

    #[doc = "Set Default Log Level"]
    pub fn set_default_level(&'a mut self, level: Level) -> &'a mut LogConfigBuilder {
        self.inner.default_level = level;
        self
    }

    #[doc = "Set Modue Log Level"]
    pub fn set_mod_level(&'a mut self, module: &str, level: Level) -> &'a mut LogConfigBuilder {
        let _dummy = self.inner.mod_level.insert(String::from(module), level);
        self
    }

    #[doc = "Set Log Destination"]
    pub fn set_log_dest(
        &'a mut self,
        dest: LogDestination,
        stream: Option<&PathBuf>,
    ) -> Result<&'a mut LogConfigBuilder, LogError> {
        if dest.is_stream_dest() {
            if let Some(stream) = stream {
                self.inner.log_stream = Some(stream.clone());
            } else {
                return Err(LogError::from_remark(
                    LogErrorKind::InvParam,
                    &format!("Missing parameter stream for log destination: {:?}", dest),
                ));
            }
        }
        self.inner.log_dest = dest;

        Ok(self)
    }

    pub fn set_no_log(&'a mut self, val: bool) {
        self.inner.no_mod = val;
    }

    pub fn build(&'a self) -> &'a LogConfig {
        &self.inner
    }


    // TODO: implement setters for all parameters
}

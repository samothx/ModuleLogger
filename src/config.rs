use failure::ResultExt;
use log::{trace, Level};
use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::Deserialize;
use serde_yaml;

use crate::{
    log_error::{LogErrCtx, LogError, LogErrorKind},
    LogDestination, DEFAULT_LOG_DEST, DEFAULT_LOG_LEVEL,
};

#[derive(Debug, Deserialize)]
pub struct LogConfig {
    default_level: Option<String>,
    mod_level: Option<HashMap<String, String>>,
    log_dest: Option<LogDestination>,
    log_stream: Option<PathBuf>,
}

impl<'a> LogConfig {
    pub fn default() -> LogConfig {
        LogConfig {
            default_level: None,
            mod_level: None,
            log_dest: Some(DEFAULT_LOG_DEST),
            log_stream: None,
        }
    }

    pub fn from_file<P: AsRef<Path>>(filename: P) -> Result<LogConfig, LogError> {
        trace!("config::from_file: entered");
        let config_path = filename.as_ref();

        let config_str = &read_to_string(&config_path).context(LogErrCtx::from_remark(
            LogErrorKind::Upstream,
            &format!(
                "config::from_file: failed to read {}",
                config_path.display()
            ),
        ))?;

        Ok(
            serde_yaml::from_str(config_str).context(LogErrCtx::from_remark(
                LogErrorKind::Upstream,
                "failed to deserialze config from yaml",
            ))?,
        )
    }

    pub fn get_default_level(&self) -> Result<Level, LogError> {
        if let Some(ref level) = self.default_level {
            Ok(Level::from_str(level).context(LogErrCtx::from_remark(
                LogErrorKind::InvParam,
                &format!("failed to parse LogLevel from '{}'", level),
            ))?)
        } else {
            Ok(DEFAULT_LOG_LEVEL)
        }
    }

    pub fn get_mod_level(&self) -> Result<HashMap<String, Level>, LogError> {
        let mut mod_level_hash: HashMap<String, Level> = HashMap::new();
        if let Some(ref mod_level) = self.mod_level {
            for (module, level_str) in mod_level {
                mod_level_hash.insert(
                    module.clone(),
                    Level::from_str(level_str).context(LogErrCtx::from_remark(
                        LogErrorKind::InvParam,
                        &format!("failed to parse LogLevel from '{}'", level_str),
                    ))?,
                );
            }
        }
        Ok(mod_level_hash)
    }

    pub fn get_log_dest(&'a self) -> &'a LogDestination {
        if let Some(ref log_dest) = self.log_dest {
            log_dest
        } else {
            &DEFAULT_LOG_DEST
        }
    }

    pub fn get_log_stream(&'a self) -> Option<&'a Path> {
        if let Some(ref log_stream) = self.log_stream {
            Some(log_stream)
        } else {
            None
        }
    }
}

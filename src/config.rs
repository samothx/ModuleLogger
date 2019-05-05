use log::{Level, debug, trace};
use yaml_rust::{Yaml, YamlLoader};
use std::path::Path;
use std::collections::HashMap;
use std::fs::{read_to_string};
use failure::{ResultExt};
use std::str::FromStr;

use crate::{
    log_error::{
        LogError, 
        LogErrorKind, 
        LogErrCtx
    },
};

pub struct LogConfig {
    pub default_level: Option<Level>,
    pub mod_level: HashMap<String, Level>,
}

impl LogConfig {
    pub fn default() -> LogConfig {
        LogConfig{
            default_level: None,
            mod_level: HashMap::new(),
        }
    }

    pub fn from_file<P: AsRef<Path>>(filename: P) -> Result<LogConfig, LogError> {
        trace!("config::from_file: entered");
        let config_path = filename.as_ref();
        let mut log_config = LogConfig::default();
        
        let config_str = &read_to_string(&config_path)
                .context(LogErrCtx::from_remark(
                    LogErrorKind::Upstream,
                    &format!("config::from_file: failed to read {}", config_path.display()),
                ))?;
            let yaml_cfg =
                YamlLoader::load_from_str(&config_str).context(LogErrCtx::from_remark(
                    LogErrorKind::Upstream,
                    "config::from_file: failed to parse",
                ))?;
            if yaml_cfg.len() > 1 {
                return Err(LogError::from_remark(
                    LogErrorKind::InvParam,
                    &format!(
                        "config::from_file: invalid number of configs in file: {}, {}",                        
                        config_path.display(),
                        yaml_cfg.len()
                    ),
                ));
            }

            if yaml_cfg.len() == 1 {
                let yaml_cfg = &yaml_cfg[0];
                if let Some(level) = get_yaml_str(yaml_cfg, &["log_level"])? {
                    if let Ok(level) = Level::from_str(level.as_ref()) {
                        log_config.default_level = Some(level);
                    }
                }

                if let Some(modules) = get_yaml_val(yaml_cfg, &["modules"])? {
                    if let Yaml::Array(ref modules) = modules {
                        for module in modules {
                            if let Some(name) = get_yaml_str(module, &["name"])? {
                                if let Some(level_str) = get_yaml_str(module, &["level"])? {
                                    if let Ok(level) = Level::from_str(level_str.as_ref()) {                                        
                                        log_config.mod_level.insert(String::from(name), level);
                                    }
                                }
                            }
                        }
                    }
                }
            }

        Ok(log_config)
    }
}

fn get_yaml_val<'a>(doc: &'a Yaml, path: &[&str]) -> Result<Option<&'a Yaml>,LogError> {
    debug!("get_yaml_val: looking for '{:?}'",  path);
    let mut last = doc;

    for comp in path {
        debug!("get_yaml_val: looking for comp: '{}'", comp);
        match last {
            Yaml::Hash(_v) => {
                let curr = &last[*comp];
                if let Yaml::BadValue = curr {
                    debug!(
                        "get_yaml_val: not found, comp: '{}' in {:?}",
                        comp, last
                    );
                    return Ok(None);
                }
                last = &curr;
            }
            _ => {
                return Err(LogError::from_remark(
                    LogErrorKind::InvParam,
                    &format!(
                        "get_yaml_val: invalid value in path, not hash for {:?}",
                        path
                    ),
                ));
            }
        }
    }

    Ok(Some(&last))
}

fn get_yaml_str<'a>(doc: &'a Yaml, path: &[&str]) -> Result<Option<&'a str>, LogError> {
    debug!("get_yaml_str: looking for '{:?}'", path);
    if let Some(value) = get_yaml_val(doc, path)? {
        match value {
            Yaml::String(s) => {
                debug!(
                    "get_yaml_str: looking for comp: {:?}, got {}",
                    path, s
                );
                Ok(Some(&s))
            }
            _ => Err(LogError::from_remark(
                LogErrorKind::InvParam,
                &format!(
                    "get_yaml_str: invalid value, not string for {:?}",
                     path
                ),
            )),
        }
    } else {
        Ok(None)
    }
}

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::Error;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claude {
    pub pretend: bool,
}

impl Default for Claude {
    fn default() -> Self {
        Claude { pretend: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Codex {
    pub pretend: bool,
}

#[allow(clippy::derivable_impls)] // We might add more configurations later, so supress this lint for now.
impl Default for Codex {
    fn default() -> Self {
        Codex { pretend: false }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub version: u32,
    pub claude: Claude,
    pub codex: Codex,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            version: 1,
            claude: Claude::default(),
            codex: Codex::default(),
        }
    }
}

pub fn get_config_path() -> Option<PathBuf> {
    let system_config_path = dirs::config_dir();

    if let Some(mut path) = system_config_path {
        path.push("agent_notifications/a-notifications.json");
        return Some(path);
    }

    let mut current_dir = env::current_dir().ok()?;

    current_dir.push("a-notifications.json");

    Some(current_dir)
}

pub fn get_logs_dir() -> PathBuf {
    if let Some(config_file) = get_config_path()
        && let Some(parent) = config_file.parent()
    {
        return parent.join("logs");
    }

    let base = dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("agent_notifications");
    base.join("logs")
}

pub fn create_default_config(path: &Path) -> Result<(), Error> {
    let default_config = Config::default();
    let config_data = serde_json::to_string(&default_config)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    std::fs::write(path, config_data)?;

    Ok(())
}

pub fn initialize_configuration(config_path: &Path) -> Result<Config, Error> {
    if !config_path.exists() {
        create_default_config(config_path)?;
    }

    let contents = fs::read_to_string(config_path)?;

    let config: Config = serde_json::from_str(&contents)?;

    Ok(config)
}

pub fn reset_configuration(config_path: &Path) -> Result<(), Error> {
    if config_path.exists() {
        fs::remove_file(config_path)?;
    }

    create_default_config(config_path)?;

    Ok(())
}

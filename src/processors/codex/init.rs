use std::{
    fmt,
    path::{Path, PathBuf},
};

use anyhow::Error;
use inquire::{Confirm, InquireError, Select};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, instrument, warn};
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
struct CodexConfiguration {
    #[serde(default)]
    notify: Option<Vec<String>>,
    #[serde(flatten)]
    other: toml::value::Table,
}

impl CodexConfiguration {
    fn set_notify(&mut self, cmd: Vec<String>) {
        self.notify = Some(cmd);
    }

    fn clear_notify(&mut self) {
        self.notify = None;
    }
}

fn handle_inquire_error(err: InquireError, context: &str) -> Error {
    match err {
        InquireError::OperationCanceled => Error::msg("Operation cancelled by user"),
        InquireError::OperationInterrupted => Error::msg("Operation interrupted by user"),
        _ => Error::msg(format!("{}: {}", context, err)),
    }
}

enum CodexConfigPathSelection {
    CodexHomeConfig(bool),
    DotCodexConfig(bool),
    CustomPath,
}

#[derive(Clone, Copy)]
enum ExistingNotifyAction {
    Override,
    Keep,
    Remove,
}

impl fmt::Display for ExistingNotifyAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExistingNotifyAction::Override => write!(f, "Override with this tool's settings"),
            ExistingNotifyAction::Keep => write!(f, "Keep it unchanged"),
            ExistingNotifyAction::Remove => write!(f, "Remove the notify configuration"),
        }
    }
}

impl fmt::Display for CodexConfigPathSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodexConfigPathSelection::CodexHomeConfig(exists) => {
                let status = if *exists { "✓" } else { "✗" };
                write!(f, "{} $CODEX_HOME/config.toml", status)
            }
            CodexConfigPathSelection::DotCodexConfig(exists) => {
                let status = if *exists { "✓" } else { "✗" };
                write!(f, "{} ~/.codex/config.toml", status)
            }
            CodexConfigPathSelection::CustomPath => write!(f, "📂 Custom Path"),
        }
    }
}

#[instrument(skip(codex_config_path))]
pub fn initialize_codex_configuration(codex_config_path: &Option<PathBuf>) -> Result<(), Error> {
    let chosen_path = choose_config_path(codex_config_path)?;
    let expanded_path = expand_tilde(&chosen_path);

    debug!(chosen = %chosen_path.display(), expanded = %expanded_path.display(), "resolved Codex config path");
    ensure_path_exists(&expanded_path)?;

    let mut config = read_config(&expanded_path)?;
    let notify_cmd = notify_command()?;

    if let Some(current) = &config.notify {
        info!(?current, "existing Codex notify configuration detected");
        println!("📋 Current notify configuration:");
        println!("  • notify = {:?}", current);
        println!();

        let choice = Select::new(
            "Notify is already configured. What would you like to do?",
            vec![
                ExistingNotifyAction::Override,
                ExistingNotifyAction::Keep,
                ExistingNotifyAction::Remove,
            ],
        )
        .with_help_message("Choose how to handle the existing notify setting")
        .prompt()
        .map_err(|err| handle_inquire_error(err, "Failed to prompt for notify action"))?;

        match choice {
            ExistingNotifyAction::Override => {
                config.set_notify(notify_cmd);
                write_config(&expanded_path, &config)?;
                info!(path = %expanded_path.display(), "overrode notify configuration");
                println!("✅ Updated: notify now uses this tool");
                println!("📁 Configuration written to: {}", expanded_path.display());
            }
            ExistingNotifyAction::Keep => {
                info!("kept existing notify configuration");
                println!("ℹ️  Keeping existing notify setting. No changes made.");
            }
            ExistingNotifyAction::Remove => {
                config.clear_notify();
                write_config(&expanded_path, &config)?;
                info!(path = %expanded_path.display(), "removed notify configuration");
                println!("🧹 Removed notify configuration");
                println!("📁 Configuration written to: {}", expanded_path.display());
            }
        }
    } else {
        let should_set = Confirm::new("Configure Codex notify to use this tool?")
            .with_default(true)
            .prompt()
            .map_err(|err| handle_inquire_error(err, "Failed to get confirmation"))?;

        if should_set {
            config.set_notify(notify_cmd);
            write_config(&expanded_path, &config)?;

            info!(path = %expanded_path.display(), "configured notify with this tool");
            println!("✅ Successfully configured notify");
            println!("📁 Configuration written to: {}", expanded_path.display());
        } else {
            info!("user declined to configure notify");
            println!("ℹ️  No changes made.");
        }
    }

    Ok(())
}

#[instrument(skip(codex_config_path))]
fn choose_config_path(codex_config_path: &Option<PathBuf>) -> Result<PathBuf, Error> {
    if let Some(p) = codex_config_path {
        info!(path = %p.display(), "using provided path");
        return Ok(p.clone());
    }

    let codex_home_dir = std::env::var("CODEX_HOME")
        .ok()
        .unwrap_or("~/.codex".to_string());
    let codex_home_path = expand_tilde(&PathBuf::from(codex_home_dir)).join("config.toml");
    let dot_codex_path = expand_tilde(&PathBuf::from("~/.codex/config.toml"));

    let codex_home_exists = codex_home_path.exists();
    let dot_codex_exists = dot_codex_path.exists();

    let selection = Select::new(
        "Where do you want to initialize the notifications for?",
        vec![
            CodexConfigPathSelection::CodexHomeConfig(codex_home_exists),
            CodexConfigPathSelection::DotCodexConfig(dot_codex_exists),
            CodexConfigPathSelection::CustomPath,
        ],
    )
    .with_help_message("Select the configuration path for Codex. ✓ = file exists, ✗ = file missing")
    .prompt()
    .map_err(|err| handle_inquire_error(err, "Failed to prompt for Codex configuration path"))?;

    let path = match selection {
        CodexConfigPathSelection::CodexHomeConfig(_) => {
            info!(path = %codex_home_path.display(), exists = codex_home_exists, "selected $CODEX_HOME/config.toml");
            codex_home_path
        }
        CodexConfigPathSelection::DotCodexConfig(_) => {
            info!(path = %dot_codex_path.display(), exists = dot_codex_exists, "selected ~/.codex/config.toml");
            dot_codex_path
        }
        CodexConfigPathSelection::CustomPath => {
            let custom_path: String = inquire::Text::new("Enter the custom path:")
                .with_help_message("Provide the full path to the Codex config.toml file.")
                .prompt()
                .map_err(|err| handle_inquire_error(err, "Failed to prompt for custom path"))?;

            info!(path = %custom_path, "selected custom path");
            PathBuf::from(custom_path)
        }
    };

    Ok(path)
}

fn expand_tilde(path: &Path) -> PathBuf {
    if let Ok(s) = path.to_path_buf().into_os_string().into_string() {
        if let Some(rest) = s.strip_prefix("~/")
            && let Ok(home) = std::env::var("HOME")
        {
            debug!(original = %s, expanded = %PathBuf::from(home.clone()).join(rest).display(), "expanding ~ to HOME");
            return PathBuf::from(home).join(rest);
        }
        return PathBuf::from(s);
    }
    path.to_path_buf()
}

#[instrument]
fn ensure_path_exists(path: &PathBuf) -> Result<(), Error> {
    if !path.exists() {
        let should_create = Confirm::new(&format!(
            "The configuration file '{}' does not exist. Would you like to create it?",
            path.display()
        ))
        .with_default(true)
        .prompt()
        .map_err(|err| handle_inquire_error(err, "Failed to get user confirmation"))?;

        if !should_create {
            info!(path = %path.display(), "user declined to create file");
            return Err(Error::msg("Operation cancelled by user"));
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .or(Err(Error::msg("Failed to create parent directories")))?;
        }

        std::fs::write(path, "").or(Err(Error::msg("Failed to create configuration file")))?;
        info!(path = %path.display(), "created empty config file");
    }
    Ok(())
}

#[instrument]
fn read_config(path: &PathBuf) -> Result<CodexConfiguration, Error> {
    let config_data = std::fs::read_to_string(path)
        .map_err(|e| Error::msg(format!("Failed to read the configuration file: {}", e)))?;

    if config_data.trim().is_empty() {
        debug!(path = %path.display(), "empty file; using default configuration");
        return Ok(CodexConfiguration::default());
    }

    let config: CodexConfiguration = toml::from_str(&config_data).map_err(|e| {
        Error::msg(format!(
            "Failed to parse the configuration file as TOML: {}. Content: {}",
            e, config_data
        ))
    })?;
    debug!(
        has_notify = config
            .notify
            .as_ref()
            .map(|v| !v.is_empty())
            .unwrap_or(false),
        other_keys = config.other.len(),
        "parsed Codex configuration"
    );
    Ok(config)
}

#[instrument]
fn notify_command() -> Result<Vec<String>, Error> {
    let current_exe =
        std::env::current_exe().or(Err(Error::msg("Failed to get current executable path")))?;
    let exe_str = current_exe.to_string_lossy().to_string();
    let cmd = vec![exe_str, "codex".to_string()];
    debug!(?cmd, "constructed notify command");
    Ok(cmd)
}

#[instrument]
fn write_config(path: &PathBuf, config: &CodexConfiguration) -> Result<(), Error> {
    let new_config = toml::to_string_pretty(config).or(Err(Error::msg(
        "Failed to serialize the configuration to TOML",
    )))?;
    std::fs::write(path, new_config)
        .or(Err(Error::msg("Failed to write the configuration file")))?;
    info!(path = %path.display(), "wrote Codex configuration");
    Ok(())
}

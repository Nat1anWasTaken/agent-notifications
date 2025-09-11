use std::{
    fmt,
    path::{Path, PathBuf},
};

use anyhow::Error;
use inquire::{Confirm, InquireError, Select};
use serde::{Deserialize, Serialize};
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

pub fn initialize_codex_configuration(codex_config_path: &Option<PathBuf>) -> Result<(), Error> {
    let chosen_path = choose_config_path(codex_config_path)?;
    let expanded_path = expand_tilde(&chosen_path);

    ensure_path_exists(&expanded_path)?;

    let mut config = read_config(&expanded_path)?;
    let notify_cmd = notify_command()?;

    if let Some(current) = &config.notify {
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
                println!("✅ Updated: notify now uses this tool");
                println!("📁 Configuration written to: {}", expanded_path.display());
            }
            ExistingNotifyAction::Keep => {
                println!("ℹ️  Keeping existing notify setting. No changes made.");
            }
            ExistingNotifyAction::Remove => {
                config.clear_notify();
                write_config(&expanded_path, &config)?;
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

            println!("✅ Successfully configured notify");
            println!("📁 Configuration written to: {}", expanded_path.display());
        } else {
            println!("ℹ️  No changes made.");
        }
    }

    Ok(())
}

fn choose_config_path(codex_config_path: &Option<PathBuf>) -> Result<PathBuf, Error> {
    if let Some(p) = codex_config_path {
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
        CodexConfigPathSelection::CodexHomeConfig(_) => codex_home_path,
        CodexConfigPathSelection::DotCodexConfig(_) => dot_codex_path,
        CodexConfigPathSelection::CustomPath => {
            let custom_path: String = inquire::Text::new("Enter the custom path:")
                .with_help_message("Provide the full path to the Codex config.toml file.")
                .prompt()
                .map_err(|err| handle_inquire_error(err, "Failed to prompt for custom path"))?;

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
            return PathBuf::from(home).join(rest);
        }
        return PathBuf::from(s);
    }
    path.to_path_buf()
}

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
            return Err(Error::msg("Operation cancelled by user"));
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .or(Err(Error::msg("Failed to create parent directories")))?;
        }

        std::fs::write(path, "").or(Err(Error::msg("Failed to create configuration file")))?;
    }
    Ok(())
}

fn read_config(path: &PathBuf) -> Result<CodexConfiguration, Error> {
    let config_data = std::fs::read_to_string(path)
        .map_err(|e| Error::msg(format!("Failed to read the configuration file: {}", e)))?;

    if config_data.trim().is_empty() {
        return Ok(CodexConfiguration::default());
    }

    let config: CodexConfiguration = toml::from_str(&config_data).map_err(|e| {
        Error::msg(format!(
            "Failed to parse the configuration file as TOML: {}. Content: {}",
            e, config_data
        ))
    })?;
    Ok(config)
}

fn notify_command() -> Result<Vec<String>, Error> {
    let current_exe =
        std::env::current_exe().or(Err(Error::msg("Failed to get current executable path")))?;
    let exe_str = current_exe.to_string_lossy().to_string();
    Ok(vec![exe_str, "codex".to_string()])
}

fn write_config(path: &PathBuf, config: &CodexConfiguration) -> Result<(), Error> {
    let new_config = toml::to_string_pretty(config).or(Err(Error::msg(
        "Failed to serialize the configuration to TOML",
    )))?;
    std::fs::write(path, new_config)
        .or(Err(Error::msg("Failed to write the configuration file")))?;
    Ok(())
}

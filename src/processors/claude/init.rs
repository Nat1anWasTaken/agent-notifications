use std::{collections::HashMap, fmt, path::PathBuf};

use anyhow::Error;
use inquire::{Confirm, Select};
use serde::{Deserialize, Serialize};

use crate::processors::claude::structs::HookEventName;
use strum::IntoEnumIterator;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
enum HookType {
    Command,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
struct ActionConfiguration {
    r#type: HookType,
    command: String,
    timeout: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
struct EventHookConfiguration {
    matcher: String,
    hooks: Vec<ActionConfiguration>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct ClaudeConfiguration {
    hooks: HashMap<HookEventName, Vec<EventHookConfiguration>>,
    #[serde(flatten)]
    other: HashMap<String, serde_json::Value>,
}

enum ClaudeCodePathSelection {
    UserSettings(bool),
    ProjectSettings(bool),
    LocalProjectSettings(bool),
    CustomPath,
}

impl fmt::Display for ClaudeCodePathSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClaudeCodePathSelection::UserSettings(exists) => {
                let status = if *exists { "✓" } else { "✗" };
                write!(f, "{} User Settings (~/.claude/settings.json)", status)
            }
            ClaudeCodePathSelection::ProjectSettings(exists) => {
                let status = if *exists { "✓" } else { "✗" };
                write!(f, "{} Project Settings (.claude/settings.json)", status)
            }
            ClaudeCodePathSelection::LocalProjectSettings(exists) => {
                let status = if *exists { "✓" } else { "✗" };
                write!(
                    f,
                    "{} Local Project Settings (.claude/settings.local.json)",
                    status
                )
            }
            ClaudeCodePathSelection::CustomPath => write!(f, "📂 Custom Path"),
        }
    }
}

pub fn initialize_claude_configuration(
    claude_config_path: &Option<PathBuf>,
) -> Result<(), anyhow::Error> {
    let chosen_path = choose_config_path(claude_config_path)?;
    let expanded_path = expand_tilde(&chosen_path);
    ensure_path_exists(&expanded_path)?;

    let config = read_config(&expanded_path)?;
    let command = agent_command()?;
    let updated = with_notification_hook(config, command);
    write_config(&expanded_path, &updated)?;

    println!("✅ Successfully configured Claude Code notifications");
    println!("📁 Configuration written to: {}", expanded_path.display());

    Ok(())
}

fn choose_config_path(claude_config_path: &Option<PathBuf>) -> Result<PathBuf, Error> {
    if let Some(p) = claude_config_path {
        return Ok(p.clone());
    }

    let user_settings_path = expand_tilde(&PathBuf::from("~/.claude/settings.json"));
    let project_settings_path = PathBuf::from(".claude/settings.json");
    let local_project_settings_path = PathBuf::from(".claude/settings.local.json");

    let user_exists = user_settings_path.exists();
    let project_exists = project_settings_path.exists();
    let local_project_exists = local_project_settings_path.exists();

    let selection = Select::new(
        "Where do you want to initialize the notifications for?",
        vec![
            ClaudeCodePathSelection::UserSettings(user_exists),
            ClaudeCodePathSelection::ProjectSettings(project_exists),
            ClaudeCodePathSelection::LocalProjectSettings(local_project_exists),
            ClaudeCodePathSelection::CustomPath,
        ],
    )
    .with_help_message(
        "Select the configuration path for Claude Code. ✓ = file exists, ✗ = file missing",
    )
    .prompt()
    .or(Err(Error::msg(
        "Failed to prompt for Claude configuration path",
    )))?;

    let path = match selection {
        ClaudeCodePathSelection::UserSettings(_) => PathBuf::from("~/.claude/settings.json"),
        ClaudeCodePathSelection::ProjectSettings(_) => PathBuf::from(".claude/settings.json"),
        ClaudeCodePathSelection::LocalProjectSettings(_) => {
            PathBuf::from(".claude/settings.local.json")
        }
        ClaudeCodePathSelection::CustomPath => {
            let custom_path: String = inquire::Text::new("Enter the custom path:")
                .with_help_message("Provide the full path to the Claude Code settings.json file.")
                .prompt()
                .or(Err(Error::msg("Failed to prompt for custom path")))?;

            PathBuf::from(custom_path)
        }
    };

    Ok(path)
}

fn expand_tilde(path: &PathBuf) -> PathBuf {
    if let Ok(s) = path.clone().into_os_string().into_string() {
        if let Some(rest) = s.strip_prefix("~/") {
            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home).join(rest);
            }
        }
        return PathBuf::from(s);
    }
    path.clone()
}

fn ensure_path_exists(path: &PathBuf) -> Result<(), Error> {
    if !path.exists() {
        let should_create = Confirm::new(&format!(
            "The configuration file '{}' does not exist. Would you like to create it?",
            path.display()
        ))
        .with_default(true)
        .prompt()
        .or(Err(Error::msg("Failed to get user confirmation")))?;

        if !should_create {
            return Err(Error::msg("Operation cancelled by user"));
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .or(Err(Error::msg("Failed to create parent directories")))?;
        }

        let empty_config = ClaudeConfiguration {
            hooks: HashMap::new(),
            other: HashMap::new(),
        };

        let config_json = serde_json::to_string_pretty(&empty_config)
            .or(Err(Error::msg("Failed to serialize empty configuration")))?;

        std::fs::write(path, config_json)
            .or(Err(Error::msg("Failed to create configuration file")))?;
    }
    Ok(())
}

fn read_config(path: &PathBuf) -> Result<ClaudeConfiguration, Error> {
    let config_data = std::fs::read_to_string(path)
        .or(Err(Error::msg("Failed to read the configuration file")))?;

    let config: ClaudeConfiguration = serde_json::from_str(&config_data)
        .or(Err(Error::msg("Failed to parse the configuration file")))?;
    Ok(config)
}

fn agent_command() -> Result<String, Error> {
    let current_exe =
        std::env::current_exe().or(Err(Error::msg("Failed to get current executable path")))?;
    let exe_str = current_exe.to_string_lossy().to_string();
    Ok(format!("\"{}\" claude", exe_str))
}

fn with_notification_hook(mut config: ClaudeConfiguration, command: String) -> ClaudeConfiguration {
    let hook_config = EventHookConfiguration {
        matcher: "".to_string(),
        hooks: vec![ActionConfiguration {
            r#type: HookType::Command,
            command: command.clone(),
            timeout: Some(10),
        }],
    };

    for event in HookEventName::iter() {
        config.hooks.insert(event, vec![hook_config.clone()]);
    }

    config
}

fn write_config(path: &PathBuf, config: &ClaudeConfiguration) -> Result<(), Error> {
    let new_config = serde_json::to_string_pretty(config)
        .or(Err(Error::msg("Failed to serialize the configuration")))?;
    std::fs::write(path, new_config)
        .or(Err(Error::msg("Failed to write the configuration file")))?;
    Ok(())
}

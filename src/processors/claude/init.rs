use std::{collections::HashMap, fmt, path::PathBuf};

use anyhow::Error;
use inquire::{Confirm, MultiSelect, Select, InquireError};
use serde::{Deserialize, Serialize};

use crate::processors::claude::structs::HookEventName;
use strum::IntoEnumIterator;

fn handle_inquire_error(err: InquireError, context: &str) -> Error {
    match err {
        InquireError::OperationCanceled => Error::msg("Operation cancelled by user"),
        InquireError::OperationInterrupted => Error::msg("Operation interrupted by user"),
        _ => Error::msg(format!("{}: {}", context, err)),
    }
}

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
    #[serde(default)]
    hooks: HashMap<HookEventName, Vec<EventHookConfiguration>>,
    #[serde(flatten)]
    other: HashMap<String, serde_json::Value>,
}

impl Default for ClaudeConfiguration {
    fn default() -> Self {
        ClaudeConfiguration {
            hooks: HashMap::new(),
            other: HashMap::new(),
        }
    }
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
    let config_exists = expanded_path.exists();
    
    ensure_path_exists(&expanded_path)?;

    let mut config = read_config(&expanded_path)?;
    let command = agent_command()?;

    if config_exists && !config.hooks.is_empty() {
        println!("📋 Current hook configuration:");
        for (hook, configurations) in &config.hooks {
            println!("  • {}: {} hook(s) configured", format!("{:?}", hook), configurations.len());
        }
        println!();
    }
    
    let selected_hooks = choose_hooks(&config)?;
    config = with_selected_notification_hooks(config, command, selected_hooks);
    write_config(&expanded_path, &config)?;

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
    .map_err(|err| handle_inquire_error(err, "Failed to prompt for Claude configuration path"))?;

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
                .map_err(|err| handle_inquire_error(err, "Failed to prompt for custom path"))?;

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
        .map_err(|err| handle_inquire_error(err, "Failed to get user confirmation"))?;

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
        .map_err(|e| Error::msg(format!("Failed to read the configuration file: {}", e)))?;

    let config: ClaudeConfiguration = serde_json::from_str(&config_data)
        .map_err(|e| Error::msg(format!("Failed to parse the configuration file: {}. Content: {}", e, config_data)))?;
    Ok(config)
}

fn is_our_notification_action(action: &ActionConfiguration) -> bool {
    action.command.contains("anot") && action.command.contains("claude")
}

fn has_our_notification_hook(event_hooks: &[EventHookConfiguration]) -> bool {
    event_hooks
        .iter()
        .any(|hook_config| hook_config.hooks.iter().any(is_our_notification_action))
}

fn get_currently_configured_hooks(config: &ClaudeConfiguration) -> Vec<HookEventName> {
    config.hooks
        .iter()
        .filter_map(|(hook_name, event_hooks)| {
            has_our_notification_hook(event_hooks)
                .then_some(hook_name.clone())
        })
        .collect()
}

fn create_hook_option(hook: &HookEventName, currently_configured: &[HookEventName]) -> String {
    let configured_marker = if currently_configured.contains(hook) { "✓" } else { " " };
    format!("[{}] {}", configured_marker, hook)
}

fn find_default_indices(currently_configured: &[HookEventName], all_hooks: &[HookEventName]) -> Vec<usize> {
    currently_configured
        .iter()
        .filter_map(|hook| all_hooks.iter().position(|h| h == hook))
        .collect()
}

fn prompt_user_selection(options: &[String], default_indices: &[usize]) -> Result<Vec<String>, Error> {
    MultiSelect::new(
        "Select which hooks you want to configure for notifications:",
        options.to_vec(),
    )
    .with_help_message("Use space to select/deselect, arrow keys to navigate, enter to confirm. [✓] = currently configured")
    .with_default(default_indices)
    .prompt()
    .map_err(|err| handle_inquire_error(err, "Failed to get hook selection"))
}

fn convert_selections_to_hooks(selected_strings: Vec<String>, options: &[String], all_hooks: &[HookEventName]) -> Vec<HookEventName> {
    selected_strings
        .into_iter()
        .filter_map(|selected_string| {
            options
                .iter()
                .position(|option| option == &selected_string)
                .map(|index| all_hooks[index].clone())
        })
        .collect()
}

fn choose_hooks(config: &ClaudeConfiguration) -> Result<Vec<HookEventName>, Error> {
    let all_hooks: Vec<HookEventName> = HookEventName::iter().collect();
    let currently_configured = get_currently_configured_hooks(config);
    
    let options: Vec<String> = all_hooks
        .iter()
        .map(|hook| create_hook_option(hook, &currently_configured))
        .collect();
    
    let default_indices = find_default_indices(&currently_configured, &all_hooks);
    let selected_strings = prompt_user_selection(&options, &default_indices)?;
    
    Ok(convert_selections_to_hooks(selected_strings, &options, &all_hooks))
}

fn agent_command() -> Result<String, Error> {
    let current_exe =
        std::env::current_exe().or(Err(Error::msg("Failed to get current executable path")))?;
    let exe_str = current_exe.to_string_lossy().to_string();
    Ok(format!("\"{}\" claude", exe_str))
}

fn create_our_hook_config(command: String) -> EventHookConfiguration {
    EventHookConfiguration {
        matcher: "".to_string(),
        hooks: vec![ActionConfiguration {
            r#type: HookType::Command,
            command,
            timeout: Some(10),
        }],
    }
}

fn remove_our_notification_hooks(config: &mut ClaudeConfiguration) {
    for (_, event_hooks) in config.hooks.iter_mut() {
        event_hooks.retain(|hook_config| {
            !hook_config.hooks.iter().any(is_our_notification_action)
        });
    }
}

fn add_hooks_to_selected_events(
    config: &mut ClaudeConfiguration, 
    selected_hooks: Vec<HookEventName>, 
    our_hook_config: EventHookConfiguration
) {
    for event in selected_hooks {
        config.hooks
            .entry(event)
            .or_insert_with(Vec::new)
            .push(our_hook_config.clone());
    }
}

fn cleanup_empty_hook_entries(config: &mut ClaudeConfiguration) {
    config.hooks.retain(|_, event_hooks| !event_hooks.is_empty());
}

fn with_selected_notification_hooks(
    mut config: ClaudeConfiguration, 
    command: String, 
    selected_hooks: Vec<HookEventName>
) -> ClaudeConfiguration {
    let our_hook_config = create_our_hook_config(command);
    
    remove_our_notification_hooks(&mut config);
    add_hooks_to_selected_events(&mut config, selected_hooks, our_hook_config);
    cleanup_empty_hook_entries(&mut config);
    
    config
}

fn write_config(path: &PathBuf, config: &ClaudeConfiguration) -> Result<(), Error> {
    let new_config = serde_json::to_string_pretty(config)
        .or(Err(Error::msg("Failed to serialize the configuration")))?;
    std::fs::write(path, new_config)
        .or(Err(Error::msg("Failed to write the configuration file")))?;
    Ok(())
}

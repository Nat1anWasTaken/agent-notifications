use std::{
    fmt,
    path::{Path, PathBuf},
};

use anyhow::Error;
use inquire::{Confirm, InquireError, Select};
use tracing::{debug, info, instrument};

fn handle_inquire_error(err: InquireError, context: &str) -> Error {
    match err {
        InquireError::OperationCanceled => Error::msg("Operation cancelled by user"),
        InquireError::OperationInterrupted => Error::msg("Operation interrupted by user"),
        _ => Error::msg(format!("{}: {}", context, err)),
    }
}

enum OpencodePluginPathSelection {
    GlobalPlugins(bool),
    ProjectPlugins(bool),
    CustomPath,
}

#[derive(Clone, Copy)]
enum ExistingPluginAction {
    Override,
    Keep,
}

impl fmt::Display for ExistingPluginAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExistingPluginAction::Override => write!(f, "Override"),
            ExistingPluginAction::Keep => write!(f, "Keep unchanged"),
        }
    }
}

impl fmt::Display for OpencodePluginPathSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpencodePluginPathSelection::GlobalPlugins(exists) => {
                let status = if *exists { "✓" } else { "✗" };
                write!(
                    f,
                    "{} Global plugins (~/.config/opencode/plugins/anot-notifications.js)",
                    status
                )
            }
            OpencodePluginPathSelection::ProjectPlugins(exists) => {
                let status = if *exists { "✓" } else { "✗" };
                write!(
                    f,
                    "{} Project plugins (.opencode/plugins/anot-notifications.js)",
                    status
                )
            }
            OpencodePluginPathSelection::CustomPath => write!(f, "📂 Custom Path"),
        }
    }
}

#[instrument(skip(opencode_plugin_path))]
pub fn initialize_opencode_configuration(
    opencode_plugin_path: &Option<PathBuf>,
) -> Result<(), Error> {
    let chosen_path = choose_plugin_path(opencode_plugin_path)?;
    let expanded_path = expand_tilde(&chosen_path);
    debug!(chosen = %chosen_path.display(), expanded = %expanded_path.display(), "resolved OpenCode plugin path");

    ensure_parent_dir_exists(&expanded_path)?;
    let plugin_exists = expanded_path.exists();

    if plugin_exists {
        info!(path = %expanded_path.display(), "existing OpenCode plugin file detected");
        println!(
            "📋 Existing plugin file detected at: {}",
            expanded_path.display()
        );
        println!();

        let choice = Select::new(
            "A plugin file already exists. What would you like to do?",
            vec![ExistingPluginAction::Override, ExistingPluginAction::Keep],
        )
        .with_help_message("Choose how to handle the existing plugin file")
        .prompt()
        .map_err(|err| handle_inquire_error(err, "Failed to prompt for existing plugin action"))?;

        match choice {
            ExistingPluginAction::Keep => {
                println!("ℹ️  Keeping existing plugin file. No changes made.");
                return Ok(());
            }
            ExistingPluginAction::Override => {}
        }
    } else {
        let should_create = Confirm::new(&format!(
            "Create OpenCode plugin file at '{}'?",
            expanded_path.display()
        ))
        .with_default(true)
        .prompt()
        .map_err(|err| handle_inquire_error(err, "Failed to get confirmation"))?;

        if !should_create {
            return Err(Error::msg("Operation cancelled by user"));
        }
    }

    let plugin_contents = plugin_file_contents()?;
    std::fs::write(&expanded_path, plugin_contents)
        .map_err(|e| Error::msg(format!("Failed to write OpenCode plugin file: {e}")))?;

    println!("✅ Successfully configured OpenCode notifications");
    println!("📁 Plugin written to: {}", expanded_path.display());
    println!(
        "ℹ️  OpenCode loads plugins from .opencode/plugins/ (project) and ~/.config/opencode/plugins/ (global)."
    );

    Ok(())
}

#[instrument(skip(opencode_plugin_path))]
fn choose_plugin_path(opencode_plugin_path: &Option<PathBuf>) -> Result<PathBuf, Error> {
    if let Some(p) = opencode_plugin_path {
        info!(path = %p.display(), "using provided path");
        return Ok(p.clone());
    }

    let global_path = expand_tilde(&PathBuf::from(
        "~/.config/opencode/plugins/anot-notifications.js",
    ));
    let project_path = PathBuf::from(".opencode/plugins/anot-notifications.js");

    let global_exists = global_path.exists();
    let project_exists = project_path.exists();

    let selection = Select::new(
        "Where do you want to install the OpenCode plugin?",
        vec![
            OpencodePluginPathSelection::GlobalPlugins(global_exists),
            OpencodePluginPathSelection::ProjectPlugins(project_exists),
            OpencodePluginPathSelection::CustomPath,
        ],
    )
    .with_help_message("✓ = file exists, ✗ = file missing")
    .prompt()
    .map_err(|err| handle_inquire_error(err, "Failed to prompt for OpenCode plugin path"))?;

    let path = match selection {
        OpencodePluginPathSelection::GlobalPlugins(_) => {
            PathBuf::from("~/.config/opencode/plugins/anot-notifications.js")
        }
        OpencodePluginPathSelection::ProjectPlugins(_) => {
            PathBuf::from(".opencode/plugins/anot-notifications.js")
        }
        OpencodePluginPathSelection::CustomPath => {
            let custom_path: String = inquire::Text::new("Enter the custom path:")
                .with_help_message(
                    "Provide the full path to the OpenCode plugin .js file (e.g. ~/.config/opencode/plugins/anot-notifications.js)",
                )
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

fn ensure_parent_dir_exists(path: &Path) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| Error::msg(format!("Failed to create parent directories: {e}")))?;
    }
    Ok(())
}

fn plugin_file_contents() -> Result<String, Error> {
    let current_exe =
        std::env::current_exe().map_err(|_| Error::msg("Failed to get current executable path"))?;
    let exe_str = current_exe.to_string_lossy().to_string();

    Ok(format!(
        "export const AgentNotificationsPlugin = async ({{ $, project, client, directory, worktree }}) => {{\n  return {{\n    event: async ({{ event }}) => {{\n      if (!event || !event.type) return\n      const supported = new Set([\"session.idle\", \"session.error\", \"permission.updated\", \"permission.asked\", \"permission.replied\"])\n      if (!supported.has(event.type)) return\n      try {{\n        await $`{exe} opencode ${{JSON.stringify(event)}}`\n      }} catch (e) {{\n        // Swallow to avoid breaking OpenCode on notification failures\n      }}\n    }},\n  }}\n}}\n",
        exe = exe_str.replace('`', "\\`")
    ))
}

use anyhow::Error;
use notify_rust::Notification;

use crate::{
    configuration::Config,
    processors::claude::structs::{HookEventName, HookInput, HookOutput, SessionEndReason},
};

pub fn process_claude_input(input: String, config: &Config) -> Result<(), Error> {
    let hook_input = match serde_json::from_str::<HookInput>(&input) {
        Ok(hook_input) => hook_input,
        Err(error) => {
            let output = HookOutput {
                system_message: Some(format!(
                    "Failed to parse input JSON: {input:?}, error: {error:?}"
                )),
                suppress_output: Some(false),
                ..Default::default()
            };

            print!("{}", serde_json::to_string(&output)?);

            return Err(Error::msg("Failed to parse input JSON"));
        }
    };

    let output = match send_notification(&hook_input, &config) {
        Ok(_) => HookOutput {
            r#continue: Some(true),
            suppress_output: Some(true),
            ..Default::default()
        },
        Err(error) => {
            let output = HookOutput {
                r#continue: Some(true),
                suppress_output: Some(true),
                system_message: Some(format!("Failed to send notification: {error:?}")),
                ..Default::default()
            };

            print!(
                "{}",
                serde_json::to_string(&output).expect("Failed to serialize output")
            );

            return Err(error);
        }
    };

    print!(
        "{}",
        serde_json::to_string(&output).expect("Failed to serialize output")
    );

    return Ok(());
}

pub fn send_notification(hook_input: &HookInput, config: &Config) -> Result<(), Error> {
    // If the hook event is not allowed by the configuration, treat as a no-op.
    if !config
        .claude
        .allowed_hooks
        .get(&hook_input.hook_event_name)
        .copied()
        .unwrap_or(false)
    {
        return Ok(());
    }

    match hook_input.hook_event_name {
        HookEventName::PreToolUse => {
            let tool_name = hook_input.tool_name.as_deref().unwrap_or("a unknown tool");

            Notification::new()
                .summary("Claude Code")
                .body(format!("The agent is trying to use {}", tool_name).as_str())
                .show()?;
        }
        HookEventName::PostToolUse => {
            let tool_name = hook_input.tool_name.as_deref().unwrap_or("a unknown tool");

            Notification::new()
                .summary("Claude Code")
                .body(format!("The agent has used {}", tool_name).as_str())
                .show()?;
        }
        HookEventName::Notification => {
            let message = hook_input
                .message
                .as_deref()
                .unwrap_or("The agent didn't provide any message.");

            Notification::new()
                .summary("Claude Code")
                .body(message)
                .show()?;
        }
        HookEventName::UserPromptSubmit => {
            let prompt = hook_input.prompt.as_deref().unwrap_or("unknown");

            Notification::new()
                .summary("Claude Code")
                .body(format!("User prompt submitted: {}", prompt).as_str())
                .show()?;
        }
        HookEventName::Stop => {
            Notification::new()
                .summary("Claude Code")
                .body("The agent has stopped responding.")
                .show()?;
        }
        HookEventName::SubagentStop => {
            Notification::new()
                .summary("Claude Code")
                .body("A subagent has stopped responding.")
                .show()?;
        }
        HookEventName::PreCompact => {
            let trigger = hook_input
                .trigger
                .as_ref()
                .map(|t| format!("{:?}", t))
                .unwrap_or_else(|| "unknown".to_string());

            Notification::new()
                .summary("Claude Code")
                .body(
                    format!(
                        "The agent is about to compact the conversation. Trigger: {}",
                        trigger
                    )
                    .as_str(),
                )
                .show()?;
        }
        HookEventName::SessionStart => {
            Notification::new()
                .summary("Claude Code")
                .body("The agent has started a new session.")
                .show()?;
        }
        HookEventName::SessionEnd => {
            let reason = match hook_input.reason.as_ref().map(|r| match r {
                SessionEndReason::Clear => "the user ran /clear.",
                SessionEndReason::PromptInputExit => {
                    "the user exited while prompt input was visible."
                }
                SessionEndReason::Logout => "the user logged out.",
                SessionEndReason::Other => "the session ended for unspecified reason.",
            }) {
                Some(r) => r,
                None => "unknown",
            };

            Notification::new()
                .summary("Claude Code")
                .body(format!("The agent has ended the session because {}", reason).as_str())
                .show()?;
        }
    }

    Ok(())
}

use anyhow::Error;
#[cfg(target_os = "macos")]
use mac_notification_sys::{get_bundle_identifier, set_application, Notification};
#[cfg(not(target_os = "macos"))]
use notify_rust::Notification;
use tracing::{debug, error, info, warn, instrument};

use crate::{
    configuration::Config,
    processors::claude::{
        icon::get_claude_icon_temp_path,
        structs::{HookEventName, HookInput, HookOutput, SessionEndReason},
    },
};

fn create_claude_notification(
    body: &str,
    #[cfg_attr(not(target_os = "macos"), allow(unused_variables))] config: &Config,
) -> Result<(), Error> {
    debug!(
        body_len = body.len(),
        pretend = config.claude.pretend,
        "preparing Claude notification"
    );
    #[cfg(target_os = "macos")]
    {
        let mut notification = Notification::new();

        notification.title("Claude Code").message(body).sound(true);

        let icon_path = get_claude_icon_temp_path().unwrap_or_default();

        if let Some(bundle_id) = get_bundle_identifier("Claude")
            && config.claude.pretend
        {
            set_application(&bundle_id).ok();
            debug!(bundle_id = %bundle_id, "using pretend app bundle for notification");
        } else {
            set_application("com.apple.Terminal").ok();
            debug!("using Terminal bundle for notification");

            if let Some(s) = icon_path.to_str() {
                notification.content_image(s);
                debug!(icon = s, "attached icon to notification");
            }
        }

        notification.send()?;
        debug!("sent macOS notification (Claude)");
    }
    #[cfg(not(target_os = "macos"))]
    {
        let mut notification = Notification::new();

        notification.summary("Claude Code").body(body);

        if let Ok(p) = get_claude_icon_temp_path()
            && let Some(s) = p.to_str()
        {
            notification.icon(s);
            debug!(icon = s, "attached icon to notification");
        }

        notification.show()?;
        debug!("sent Linux notification (Claude)");
    }
    Ok(())
}

#[instrument(skip(input, config), level = "debug")]
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

            error!(error = ?error, "failed to parse Claude input JSON");
            return Err(Error::msg("Failed to parse input JSON"));
        }
    };

    let output = match send_notification(&hook_input, config) {
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

            error!(error = ?error, "failed to send Claude notification");
            return Err(error);
        }
    };

    print!(
        "{}",
        serde_json::to_string(&output).expect("Failed to serialize output")
    );
    debug!(
        suppress_output = output.suppress_output.unwrap_or(false),
        cont = output.r#continue.unwrap_or(false),
        has_system_message = output.system_message.as_ref().map(|s| !s.is_empty()).unwrap_or(false),
        "emitted Claude hook output JSON"
    );

    Ok(())
}

#[instrument(skip(hook_input, config), fields(event = ?hook_input.hook_event_name), level = "debug")]
pub fn send_notification(hook_input: &HookInput, config: &Config) -> Result<(), Error> {
    match hook_input.hook_event_name {
        HookEventName::PreToolUse => {
            let tool_name = hook_input.tool_name.as_deref().unwrap_or("a unknown tool");
            info!(tool = tool_name, "Claude: pre tool use");

            create_claude_notification(
                &format!("The agent is trying to use {}", tool_name),
                config,
            )?
        }
        HookEventName::PostToolUse => {
            let tool_name = hook_input.tool_name.as_deref().unwrap_or("a unknown tool");
            info!(tool = tool_name, "Claude: post tool use");

            create_claude_notification(&format!("The agent has used {}", tool_name), config)?
        }
        HookEventName::Notification => {
            let message = hook_input
                .message
                .as_deref()
                .unwrap_or("The agent didn't provide any message.");
            let preview: String = message.chars().take(120).collect();
            info!("Claude: generic notification");
            debug!(message_len = message.len(), preview = preview, "constructed notification message");

            create_claude_notification(message, config)?
        }
        HookEventName::UserPromptSubmit => {
            let prompt = hook_input.prompt.as_deref().unwrap_or("unknown");
            let preview: String = prompt.chars().take(120).collect();
            info!("Claude: user prompt submitted");
            debug!(prompt_len = prompt.len(), preview = preview, "user prompt preview");

            create_claude_notification(&format!("User prompt submitted: {}", prompt), config)?
        }
        HookEventName::Stop => {
            info!("Claude: session stop");
            create_claude_notification("The agent has stopped responding.", config)?
        }
        HookEventName::SubagentStop => {
            info!("Claude: subagent stop");
            create_claude_notification("A subagent has stopped responding.", config)?
        }
        HookEventName::PreCompact => {
            let trigger = hook_input
                .trigger
                .as_ref()
                .map(|t| format!("{:?}", t))
                .unwrap_or_else(|| "unknown".to_string());
            info!("Claude: pre compact");
            debug!(trigger = trigger, "compaction trigger");

            create_claude_notification(
                &format!(
                    "The agent is about to compact the conversation. Trigger: {}",
                    trigger
                ),
                config,
            )?
        }
        HookEventName::SessionStart => {
            info!("Claude: session start");
            create_claude_notification("The agent has started a new session.", config)?
        }
        HookEventName::SessionEnd => {
            let reason = hook_input
                .reason
                .as_ref()
                .map(|r| match r {
                    SessionEndReason::Clear => "the user ran /clear.",
                    SessionEndReason::PromptInputExit => {
                        "the user exited while prompt input was visible."
                    }
                    SessionEndReason::Logout => "the user logged out.",
                    SessionEndReason::Other => "the session ended for unspecified reason.",
                })
                .unwrap_or("unknown");
            info!("Claude: session end");
            debug!(reason = reason, "session end reason");

            create_claude_notification(
                &format!("The agent has ended the session because {}", reason),
                config,
            )?
        }
    }

    Ok(())
}

// Claude input/output processing

use anyhow::Error;

use crate::{
    configuration::Config,
    processors::claude::{
        icon::get_claude_icon_temp_path,
        structs::{HookEventName, HookInput, HookOutput, SessionEndReason},
    },
};

fn create_claude_notification(body: &str) -> Result<(), Error> {
    #[cfg(target_os = "macos")]
    {
        use mac_notification_sys::Notification;
        use mac_notification_sys::get_bundle_identifier;
        use mac_notification_sys::set_application;

        let mut notification = Notification::new();

        notification.title("Claude Code").message(body).sound(true);

        let icon_path = get_claude_icon_temp_path().unwrap_or_default();

        if let Some(bundle_id) = get_bundle_identifier("Claude") {
            set_application(&bundle_id).ok();
        } else {
            set_application("com.apple.Terminal").ok();

            if let Some(s) = icon_path.to_str() {
                notification.content_image(s);
            }
        }

        notification.send()?;
    }
    #[cfg(not(target_os = "macos"))]
    {
        use notify_rust::Notification;

        let mut notification = Notification::new();

        notification.summary("Claude Code").body(body);

        if let Ok(p) = get_claude_icon_temp_path() {
            if let Some(s) = p.to_str() {
                notification.icon(s);
            }
        }

        notification.show()?;
    }
    Ok(())
}

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

            return Err(error);
        }
    };

    print!(
        "{}",
        serde_json::to_string(&output).expect("Failed to serialize output")
    );

    Ok(())
}

pub fn send_notification(hook_input: &HookInput, _config: &Config) -> Result<(), Error> {
    match hook_input.hook_event_name {
        HookEventName::PreToolUse => {
            let tool_name = hook_input.tool_name.as_deref().unwrap_or("a unknown tool");

            create_claude_notification(&format!("The agent is trying to use {}", tool_name))?
        }
        HookEventName::PostToolUse => {
            let tool_name = hook_input.tool_name.as_deref().unwrap_or("a unknown tool");

            create_claude_notification(&format!("The agent has used {}", tool_name))?
        }
        HookEventName::Notification => {
            let message = hook_input
                .message
                .as_deref()
                .unwrap_or("The agent didn't provide any message.");

            create_claude_notification(message)?
        }
        HookEventName::UserPromptSubmit => {
            let prompt = hook_input.prompt.as_deref().unwrap_or("unknown");

            create_claude_notification(&format!("User prompt submitted: {}", prompt))?
        }
        HookEventName::Stop => create_claude_notification("The agent has stopped responding.")?,
        HookEventName::SubagentStop => {
            create_claude_notification("A subagent has stopped responding.")?
        }
        HookEventName::PreCompact => {
            let trigger = hook_input
                .trigger
                .as_ref()
                .map(|t| format!("{:?}", t))
                .unwrap_or_else(|| "unknown".to_string());

            create_claude_notification(&format!(
                "The agent is about to compact the conversation. Trigger: {}",
                trigger
            ))?
        }
        HookEventName::SessionStart => {
            create_claude_notification("The agent has started a new session.")?
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

            create_claude_notification(&format!(
                "The agent has ended the session because {}",
                reason
            ))?
        }
    }

    Ok(())
}

use anyhow::Error;

use crate::{
    configuration::Config,
    processors::codex::icon::get_codex_icon_path,
    processors::codex::structs::{CodexNotificationInput, NotificationType},
};

fn create_codex_notification(
    body: &str,
    #[cfg_attr(not(target_os = "macos"), allow(unused_variables))] config: &Config,
) -> Result<(), Error> {
    #[cfg(target_os = "macos")]
    {
        use mac_notification_sys::Notification;
        use mac_notification_sys::get_bundle_identifier;
        use mac_notification_sys::set_application;

        let mut notification = Notification::new();

        notification.title("Codex").message(body).sound(true);

        let icon_path = get_codex_icon_path().unwrap_or_default();

        if let Some(bundle_id) = get_bundle_identifier("ChatGPT")
            && config.codex.pretend
        {
            set_application(&bundle_id).ok();
        } else {
            set_application("com.apple.Terminal").ok();

            if let Some(s) = icon_path.to_str() {
                notification.content_image(s);
            }
        };

        notification.send()?;
    }
    #[cfg(not(target_os = "macos"))]
    {
        use notify_rust::Notification;

        let mut notification = Notification::new();

        notification.summary("Codex").body(body);

        if let Ok(p) = get_codex_icon_path()
            && let Some(s) = p.to_str()
        {
            notification.icon(s);
        }

        notification.show()?;
    }
    Ok(())
}

pub fn process_codex_input(input: String, config: &Config) -> Result<(), Error> {
    let payload = match serde_json::from_str::<CodexNotificationInput>(&input) {
        Ok(v) => v,
        Err(e) => {
            return Err(Error::msg(format!(
                "Failed to parse Codex notification JSON: {e}"
            )));
        }
    };

    send_notification(&payload, config)
}

pub fn send_notification(
    notification: &CodexNotificationInput,
    config: &Config,
) -> Result<(), Error> {
    match notification.r#type {
        NotificationType::AgentTurnComplete => {
            let preferred_message = notification
                .last_assistant_message
                .as_ref()
                .filter(|s| !s.trim().is_empty())
                .cloned()
                .or_else(|| {
                    notification.input_messages.as_ref().map(|inputs| {
                        let joined = inputs.join(" ");
                        if joined.trim().is_empty() {
                            "Turn Complete!".to_string()
                        } else {
                            joined
                        }
                    })
                })
                .unwrap_or_else(|| "Turn Complete!".to_string());

            let body = format!("Turn Completed: {}", preferred_message);

            create_codex_notification(&body, config)?;
        }
        NotificationType::Unknown => {
            // Surface unknown events to stderr to aid debugging
            eprintln!(
                "[anot codex] Unknown notification type in payload. Fields present: turn_id={:?} last_assistant_message_present={} input_messages_len={}",
                notification.turn_id,
                notification
                    .last_assistant_message
                    .as_ref()
                    .map(|s| !s.trim().is_empty())
                    .unwrap_or(false),
                notification
                    .input_messages
                    .as_ref()
                    .map(|v| v.len())
                    .unwrap_or(0)
            );
        }
    }

    Ok(())
}

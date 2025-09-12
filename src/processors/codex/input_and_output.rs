use anyhow::Error;
#[cfg(target_os = "macos")]
use mac_notification_sys::{Notification, get_bundle_identifier, set_application};
#[cfg(not(target_os = "macos"))]
use notify_rust::Notification;
use tracing::{debug, error, info, instrument, warn};

use crate::{
    configuration::Config,
    processors::codex::icon::get_codex_icon_path,
    processors::codex::structs::{CodexNotificationInput, NotificationType},
};

fn create_codex_notification(
    summary: &str,
    body: &str,
    #[cfg_attr(not(target_os = "macos"), allow(unused_variables))] config: &Config,
) -> Result<(), Error> {
    debug!(
        body_len = body.len(),
        pretend = config.codex.pretend,
        "preparing Codex notification"
    );
    #[cfg(target_os = "macos")]
    {
        use mac_notification_sys::Notification;
        use mac_notification_sys::Sound;
        use mac_notification_sys::get_bundle_identifier;
        use mac_notification_sys::set_application;

        let mut notification = Notification::new();

        let title = format!("Codex: {}", &summary);

        notification.title(&title).message(body).sound(true);

        let icon_path = get_codex_icon_path().unwrap_or_default();

        if let Some(bundle_id) = get_bundle_identifier("ChatGPT")
            && config.codex.pretend
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
        };

        if config.codex.sound {
            notification.sound(Sound::Default);
        }

        notification.send()?;
        debug!("sent macOS notification (Codex)");
    }
    #[cfg(not(target_os = "macos"))]
    {
        let mut notification = Notification::new();

        let title = format!("Codex: {}", &summary);

        notification.summary(&title).body(body);

        if let Ok(p) = get_codex_icon_path()
            && let Some(s) = p.to_str()
        {
            notification.icon(s);
            debug!(icon = s, "attached icon to notification");
        }

        notification.show()?;
        debug!("sent Linux notification (Codex)");
    }
    Ok(())
}

#[instrument(skip(input, config), level = "debug")]
pub fn process_codex_input(input: String, config: &Config) -> Result<(), Error> {
    let payload = match serde_json::from_str::<CodexNotificationInput>(&input) {
        Ok(v) => v,
        Err(e) => {
            error!(error = %e, "failed to parse Codex notification JSON");
            return Err(Error::msg(format!(
                "Failed to parse Codex notification JSON: {e}"
            )));
        }
    };
    info!(
        event_type = ?payload.r#type,
        has_last_assistant_message = payload
            .last_assistant_message
            .as_ref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false),
        input_messages_len = payload
            .input_messages
            .as_ref()
            .map(|v| v.len())
            .unwrap_or(0),
        "parsed Codex input"
    );
    send_notification(&payload, config)
}

#[instrument(skip(notification, config), level = "debug")]
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
            let preview: String = preferred_message.chars().take(120).collect();
            info!("Codex: agent turn complete");
            debug!(
                message_len = preferred_message.len(),
                preview = preview,
                "chosen message"
            );

            create_codex_notification(notification.r#type.as_str(), &body, config)?;
        }
        NotificationType::Unknown => {
            warn!(
                turn_id = ?notification.turn_id,
                last_assistant_message_present = notification
                    .last_assistant_message
                    .as_ref()
                    .map(|s| !s.trim().is_empty())
                    .unwrap_or(false),
                input_messages_len = notification
                    .input_messages
                    .as_ref()
                    .map(|v| v.len())
                    .unwrap_or(0),
                "unknown Codex notification type"
            );
        }
    }

    Ok(())
}

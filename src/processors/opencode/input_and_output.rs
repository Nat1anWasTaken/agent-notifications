use anyhow::Error;
#[cfg(not(target_os = "macos"))]
use notify_rust::Notification;
use tracing::{debug, error, info, instrument};

use serde_json::Value;

use crate::{
    configuration::Config,
    processors::opencode::{icon::get_opencode_icon_path, structs::OpencodeSupportedEvent},
};

use super::structs::parse_supported_event;

fn create_opencode_notification(
    title: &str,
    body: &str,
    #[cfg_attr(not(target_os = "macos"), allow(unused_variables))] config: &Config,
) -> Result<(), Error> {
    debug!(body_len = body.len(), "preparing OpenCode notification");

    #[cfg(target_os = "macos")]
    {
        use mac_notification_sys::Notification;
        use mac_notification_sys::Sound;
        use mac_notification_sys::get_bundle_identifier;
        use mac_notification_sys::set_application;

        let mut notification = Notification::new();
        notification.title(title).message(body).sound(true);

        let icon_path = get_opencode_icon_path().unwrap_or_default();

        if let Some(bundle_id) = get_bundle_identifier("OpenCode")
            && config.opencode.pretend
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

        if config.opencode.sound {
            notification.sound(Sound::Default);
        }

        notification.send()?;
        debug!("sent macOS notification (OpenCode)");
    }

    #[cfg(not(target_os = "macos"))]
    {
        let mut notification = Notification::new();
        notification.summary(title).body(body);

        if let Ok(p) = get_opencode_icon_path()
            && let Some(s) = p.to_str()
        {
            notification.icon(s);
            debug!(icon = s, "attached icon to notification");
        }
        notification.show()?;
        debug!("sent Linux notification (OpenCode)");
    }

    Ok(())
}

fn map_event_to_message(event: &OpencodeSupportedEvent) -> (String, String) {
    fn push_line(lines: &mut Vec<String>, key: &str, value: impl AsRef<str>) {
        let v = value.as_ref();
        if !v.is_empty() {
            lines.push(format!("{key}: {v}"));
        }
    }

    fn display_json_value(value: &Value, max_chars: usize) -> String {
        let s = if let Some(s) = value.as_str() {
            s.to_string()
        } else {
            value.to_string()
        };

        let mut out = String::new();
        for (i, ch) in s.chars().enumerate() {
            if i >= max_chars {
                out.push('…');
                break;
            }
            out.push(ch);
        }
        out
    }

    match event {
        OpencodeSupportedEvent::SessionIdle { session_id } => {
            let mut lines = vec!["type: session.idle".to_string()];
            push_line(&mut lines, "sessionID", session_id);
            lines.push("message: Generation completed".to_string());
            ("OpenCode".to_string(), lines.join("\n"))
        }
        OpencodeSupportedEvent::Permission {
            event_type,
            permission,
        } => {
            let mut lines = vec![format!("type: {event_type}")];
            push_line(&mut lines, "sessionID", &permission.session_id);
            push_line(&mut lines, "requestID", &permission.id);

            if let Some(title) = permission.title.as_deref() {
                push_line(&mut lines, "title", title);
            }

            if let Some(p) = permission.permission.as_deref() {
                push_line(&mut lines, "permission", p);
            }
            if let Some(p) = permission.permission_type.as_deref() {
                push_line(&mut lines, "permissionType", p);
            }

            if !permission.patterns.is_empty() {
                push_line(&mut lines, "patterns", &permission.patterns.join(", "));
            }
            if !permission.always.is_empty() {
                push_line(&mut lines, "always", &permission.always.join(", "));
            }

            if let Some(tool) = permission.tool.as_ref() {
                push_line(&mut lines, "tool.messageID", &tool.message_id);
                push_line(&mut lines, "tool.callID", &tool.call_id);
            }
            if let Some(message_id) = permission.message_id.as_deref() {
                push_line(&mut lines, "messageID", message_id);
            }
            if let Some(call_id) = permission.call_id.as_deref() {
                push_line(&mut lines, "callID", call_id);
            }

            if let Some(pattern) = permission.pattern.as_ref() {
                push_line(&mut lines, "pattern", display_json_value(pattern, 400));
            }
            if let Some(time) = permission.time.as_ref() {
                push_line(&mut lines, "time.created", time.created.to_string());
            }

            if !permission.metadata.is_empty() {
                let mut pairs: Vec<_> = permission.metadata.iter().collect();
                pairs.sort_by(|a, b| a.0.cmp(b.0));

                for (k, v) in pairs {
                    push_line(
                        &mut lines,
                        &format!("metadata.{k}"),
                        display_json_value(v, 200),
                    );
                }
            }

            ("OpenCode".to_string(), lines.join("\n"))
        }
        OpencodeSupportedEvent::PermissionReplied {
            session_id,
            request_id,
            reply,
        } => {
            let mut lines = vec!["type: permission.replied".to_string()];
            push_line(&mut lines, "sessionID", session_id);
            push_line(&mut lines, "requestID", request_id);
            push_line(&mut lines, "reply", reply);
            ("OpenCode".to_string(), lines.join("\n"))
        }
        OpencodeSupportedEvent::QuestionAsked { session_id, question } => {
            let mut lines = vec!["type: question.asked".to_string()];
            if let Some(id) = session_id.as_deref() {
                push_line(&mut lines, "sessionID", id);
            }
            push_line(&mut lines, "question", question);
            ("OpenCode".to_string(), lines.join("\n"))
        }
        OpencodeSupportedEvent::SessionError {
            session_id,
            summary,
            error,
        } => {
            let mut lines = vec!["type: session.error".to_string()];
            if let Some(id) = session_id.as_deref() {
                push_line(&mut lines, "sessionID", id);
            }
            push_line(&mut lines, "summary", summary);

            if let Some(err) = error.as_ref() {
                if let Some(name) = err
                    .get("name")
                    .and_then(Value::as_str)
                    .or_else(|| err.get("type").and_then(Value::as_str))
                {
                    push_line(&mut lines, "error.name", name);
                }
                if let Some(msg) = err
                    .pointer("/data/message")
                    .and_then(Value::as_str)
                    .or_else(|| err.get("message").and_then(Value::as_str))
                {
                    push_line(&mut lines, "error.message", msg);
                }
                if let Some(provider_id) = err.pointer("/data/providerID").and_then(Value::as_str)
                {
                    push_line(&mut lines, "error.data.providerID", provider_id);
                }

                if let Some(status_code) = err
                    .pointer("/data/statusCode")
                    .and_then(Value::as_i64)
                    .or_else(|| err.pointer("/data/statusCode").and_then(Value::as_u64).map(|v| v as i64))
                {
                    push_line(&mut lines, "error.data.statusCode", status_code.to_string());
                }

                if let Some(is_retryable) = err.pointer("/data/isRetryable").and_then(Value::as_bool)
                {
                    push_line(&mut lines, "error.data.isRetryable", is_retryable.to_string());
                }

                if let Some(body) = err
                    .pointer("/data/responseBody")
                    .and_then(Value::as_str)
                    .or_else(|| err.get("responseBody").and_then(Value::as_str))
                {
                    push_line(&mut lines, "error.data.responseBody", display_json_value(&Value::String(body.to_string()), 300));
                }

                push_line(&mut lines, "error.raw", display_json_value(err, 400));
            }

            ("OpenCode".to_string(), lines.join("\n"))
        }
    }
}

#[instrument(skip(input, config), level = "debug")]
pub fn process_opencode_input(input: String, config: &Config) -> Result<(), Error> {
    let evt = match parse_supported_event(&input) {
        Ok(Some(v)) => v,
        Ok(None) => {
            info!("OpenCode: unhandled event type; no-op");
            return Ok(());
        }
        Err(e) => {
            error!(error = %e, "failed to parse/validate OpenCode event");
            return Err(e);
        }
    };

    match &evt {
        OpencodeSupportedEvent::SessionIdle { session_id } => {
            info!(session_id = session_id, "OpenCode: session idle");
        }
        OpencodeSupportedEvent::Permission {
            event_type,
            permission,
        } => {
            info!(
                session_id = permission.session_id,
                permission_id = permission.id,
                event_type = event_type,
                "OpenCode: permission updated"
            );
        }
        OpencodeSupportedEvent::PermissionReplied {
            session_id,
            request_id,
            reply,
        } => {
            info!(
                session_id = session_id,
                request_id = request_id,
                reply = reply,
                "OpenCode: permission replied"
            );
        }
        OpencodeSupportedEvent::QuestionAsked { session_id, question } => {
            info!(
                session_id = ?session_id,
                question = question,
                "OpenCode: question asked"
            );
        }
        OpencodeSupportedEvent::SessionError {
            session_id,
            summary,
            error: _,
        } => {
            info!(session_id = ?session_id, summary = summary, "OpenCode: session error");
        }
    }

    let (title, body) = map_event_to_message(&evt);
    create_opencode_notification(&title, &body, config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_session_error_with_session_id() {
        let (title, body) = map_event_to_message(&OpencodeSupportedEvent::SessionError {
            session_id: Some("abc123".to_string()),
            summary: "UnknownError: boom".to_string(),
            error: None,
        });
        assert_eq!(title, "OpenCode");
        assert!(body.contains("abc123"));
        assert!(body.contains("boom"));
    }

    #[test]
    fn maps_session_idle_includes_session_id() {
        let (_title, body) = map_event_to_message(&OpencodeSupportedEvent::SessionIdle {
            session_id: "abc123".to_string(),
        });
        assert!(body.contains("session.idle"));
        assert!(body.contains("abc123"));
    }
}

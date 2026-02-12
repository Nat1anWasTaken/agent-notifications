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
    fn format_sentence(value: impl AsRef<str>) -> Option<String> {
        let trimmed = value.as_ref().trim();
        if trimmed.is_empty() {
            return None;
        }
        let last = trimmed.chars().last().unwrap_or('.');
        let needs_period = !matches!(last, '.' | '!' | '?');
        Some(if needs_period {
            format!("{trimmed}.")
        } else {
            trimmed.to_string()
        })
    }

    fn push_sentence(lines: &mut Vec<String>, value: impl AsRef<str>) {
        if let Some(sentence) = format_sentence(value) {
            lines.push(sentence);
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
            let mut lines = Vec::new();
            push_sentence(&mut lines, format!("Session {session_id} is idle"));
            ("OpenCode".to_string(), lines.join("\n"))
        }
        OpencodeSupportedEvent::Permission {
            event_type,
            permission,
        } => {
            let mut lines = Vec::new();
            let verb = match event_type.as_str() {
                "permission.asked" => "Permission request",
                "permission.updated" => "Permission update",
                _ => "Permission event",
            };
            let mut first = if permission.id.is_empty() {
                verb.to_string()
            } else {
                format!("{verb} {}", permission.id)
            };
            if let Some(p) = permission.permission.as_deref() {
                first = format!("{first} to {p}");
            }
            first = format!("{first} for session {}", permission.session_id);
            push_sentence(&mut lines, first);

            if !permission.patterns.is_empty() {
                push_sentence(
                    &mut lines,
                    format!("Applies to {}", permission.patterns.join(", ")),
                );
            }
            if !permission.always.is_empty() {
                push_sentence(
                    &mut lines,
                    format!("Always allow for {}", permission.always.join(", ")),
                );
            }

            if let Some(tool) = permission.tool.as_ref() {
                push_sentence(&mut lines, format!("From tool call {}", tool.call_id));
                push_sentence(
                    &mut lines,
                    format!("Tool message {}", tool.message_id),
                );
            }

            if !permission.metadata.is_empty() {
                let mut keys: Vec<_> = permission.metadata.keys().cloned().collect();
                keys.sort();
                push_sentence(&mut lines, format!("Metadata keys: {}", keys.join(", ")));
            }

            ("OpenCode".to_string(), lines.join("\n"))
        }
        OpencodeSupportedEvent::PermissionReplied {
            session_id,
            request_id,
            reply,
        } => {
            let mut lines = Vec::new();
            let reply_text = match reply.as_str() {
                "once" => "allowed once",
                "always" => "always allowed",
                "reject" => "rejected",
                _ => reply,
            };
            push_sentence(
                &mut lines,
                format!("Permission request {request_id} was {reply_text} for session {session_id}"),
            );
            ("OpenCode".to_string(), lines.join("\n"))
        }
        OpencodeSupportedEvent::QuestionAsked {
            session_id,
            request_id,
            questions,
            tool,
        } => {
            let mut lines = Vec::new();
            if let Some(id) = session_id.as_deref() {
                push_sentence(&mut lines, format!("Question asked in session {id}"));
            } else {
                push_sentence(&mut lines, "Question asked");
            }
            if let Some(id) = request_id.as_deref() {
                push_sentence(&mut lines, format!("Request {id}"));
            }

            if let Some(first) = questions.first() {
                if !first.header.is_empty() && !first.question.is_empty() {
                    push_sentence(
                        &mut lines,
                        format!("{}: {}", first.header, first.question),
                    );
                } else if !first.question.is_empty() {
                    push_sentence(&mut lines, format!("Question: {}", first.question));
                } else if !first.header.is_empty() {
                    push_sentence(&mut lines, format!("Question {header}", header = first.header));
                }

                if questions.len() > 1 {
                    push_sentence(&mut lines, format!("Includes {} questions", questions.len()));
                }

                if !first.options.is_empty() {
                    let labels: Vec<_> = first.options.iter().map(|o| o.label.as_str()).collect();
                    push_sentence(&mut lines, format!("Options: {}", labels.join(", ")));
                }
                if first.multiple == Some(true) {
                    push_sentence(&mut lines, "Multiple selections allowed");
                }
                if first.custom == Some(true) {
                    push_sentence(&mut lines, "Custom answers allowed");
                }
            }

            if let Some(tool) = tool.as_ref() {
                push_sentence(&mut lines, format!("From tool call {}", tool.call_id));
                push_sentence(&mut lines, format!("Tool message {}", tool.message_id));
            }

            ("OpenCode".to_string(), lines.join("\n"))
        }
        OpencodeSupportedEvent::SessionError {
            session_id,
            summary,
            error,
        } => {
            let mut lines = Vec::new();
            if let Some(id) = session_id.as_deref() {
                push_sentence(&mut lines, format!("Session {id} encountered an error"));
            } else {
                push_sentence(&mut lines, "A session error occurred");
            }
            if !summary.is_empty() {
                push_sentence(&mut lines, format!("Error: {summary}"));
            }

            if let Some(err) = error.as_ref() {
                if let Some(provider_id) = err.pointer("/data/providerID").and_then(Value::as_str)
                {
                    push_sentence(&mut lines, format!("Provider {provider_id}"));
                }

                if let Some(status_code) = err
                    .pointer("/data/statusCode")
                    .and_then(Value::as_i64)
                    .or_else(|| err.pointer("/data/statusCode").and_then(Value::as_u64).map(|v| v as i64))
                {
                    push_sentence(&mut lines, format!("Status code {status_code}"));
                }

                if let Some(is_retryable) = err.pointer("/data/isRetryable").and_then(Value::as_bool)
                {
                    let sentence = if is_retryable {
                        "Retryable"
                    } else {
                        "Not retryable"
                    };
                    push_sentence(&mut lines, sentence);
                }

                if let Some(body) = err
                    .pointer("/data/responseBody")
                    .and_then(Value::as_str)
                    .or_else(|| err.get("responseBody").and_then(Value::as_str))
                {
                    push_sentence(
                        &mut lines,
                        format!(
                            "Response: {}",
                            display_json_value(&Value::String(body.to_string()), 300)
                        ),
                    );
                }
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
        OpencodeSupportedEvent::QuestionAsked {
            session_id,
            request_id,
            questions,
            tool: _,
        } => {
            let first_question = questions.first().map(|q| q.question.as_str());
            info!(
                session_id = ?session_id,
                request_id = ?request_id,
                question_count = questions.len(),
                first_question = ?first_question,
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
        assert!(body.contains("Session abc123"));
        assert!(body.contains("UnknownError"));
    }

    #[test]
    fn maps_session_idle_includes_session_id() {
        let (_title, body) = map_event_to_message(&OpencodeSupportedEvent::SessionIdle {
            session_id: "abc123".to_string(),
        });
        assert!(body.contains("idle"));
        assert!(body.contains("abc123"));
    }
}

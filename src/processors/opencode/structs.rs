use std::collections::HashMap;

use anyhow::Error;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpencodeSupportedEvent {
    SessionIdle {
        session_id: String,
    },
    Permission {
        event_type: String,
        permission: Box<Permission>,
    },
    PermissionReplied {
        session_id: String,
        request_id: String,
        reply: String,
    },
    SessionError {
        session_id: Option<String>,
        summary: String,
        error: Option<Value>,
    },
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct PermissionTime {
    pub created: u64,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct PermissionTool {
    #[serde(rename = "messageID")]
    pub message_id: String,

    #[serde(rename = "callID")]
    pub call_id: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Permission {
    pub id: String,

    #[serde(rename = "sessionID")]
    pub session_id: String,


    #[serde(default)]
    pub permission: Option<String>,

    #[serde(default)]
    pub patterns: Vec<String>,

    #[serde(default)]
    pub always: Vec<String>,

    #[serde(default)]
    pub tool: Option<PermissionTool>,

    #[serde(rename = "type", default)]
    pub permission_type: Option<String>,

    #[serde(rename = "messageID", default)]
    pub message_id: Option<String>,

    #[serde(default)]
    pub title: Option<String>,

    #[serde(default)]
    pub time: Option<PermissionTime>,

    #[serde(default)]
    pub pattern: Option<Value>,

    #[serde(rename = "callID", default)]
    pub call_id: Option<String>,

    #[serde(default)]
    pub metadata: HashMap<String, Value>,
}

fn event_type(value: &Value) -> Result<&str, Error> {
    value
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::msg("Missing required field: type"))
}

fn event_payload(value: &Value) -> &Value {
    if let Some(payload) = value.get("payload") {
        payload
    } else if let Some(event) = value.get("event") {
        event
    } else {
        value
    }
}

fn resolve_session_id(value: &Value) -> Option<&str> {
    value.get("sessionID").and_then(Value::as_str).or_else(|| {
        value
            .pointer("/properties/sessionID")
            .and_then(Value::as_str)
    })
}

fn resolve_error_value<'a>(event: &'a Value, root: &'a Value) -> Option<&'a Value> {
    event
        .pointer("/properties/error")
        .or_else(|| event.get("error"))
        .or_else(|| root.get("error"))
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
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

fn error_summary(error: &Value) -> String {
    if let Some(s) = error.as_str() {
        return truncate_chars(s, 400);
    }

    let name = error
        .get("name")
        .and_then(Value::as_str)
        .or_else(|| error.get("type").and_then(Value::as_str));

    let message = error
        .pointer("/data/message")
        .and_then(Value::as_str)
        .or_else(|| error.get("message").and_then(Value::as_str));

    let summary = match (name, message) {
        (Some(n), Some(m)) if n == m => n.to_string(),
        (Some(n), Some(m)) => format!("{n}: {m}"),
        (Some(n), None) => n.to_string(),
        (None, Some(m)) => m.to_string(),
        (None, None) => error.to_string(),
    };

    truncate_chars(&summary, 400)
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
struct PermissionRepliedProperties {
    #[serde(rename = "sessionID")]
    pub session_id: String,

    #[serde(rename = "requestID")]
    pub request_id: String,

    pub reply: String,
}

fn strip_event_type(mut v: Value) -> Value {
    if let Some(obj) = v.as_object_mut() {
        obj.remove("type");
    }
    v
}

fn parse_permission(
    event_type: &str,
    properties: Value,
    event: &Value,
    root: &Value,
) -> Result<Permission, Error> {
    let mut patched = properties;

    let properties_has_session_id = patched
        .get("sessionID")
        .and_then(Value::as_str)
        .is_some();
    if !properties_has_session_id {
        let session_id = event
            .get("sessionID")
            .and_then(Value::as_str)
            .or_else(|| root.get("sessionID").and_then(Value::as_str));

        if let Some(session_id) = session_id
            && let Some(obj) = patched.as_object_mut()
        {
            obj.insert(
                "sessionID".to_string(),
                Value::String(session_id.to_string()),
            );
        }
    }

    serde_json::from_value::<Permission>(patched)
        .map_err(|e| Error::msg(format!("Invalid {event_type} properties: {e}")))
}

pub fn parse_supported_event(input: &str) -> Result<Option<OpencodeSupportedEvent>, Error> {
    let value: Value =
        serde_json::from_str(input).map_err(|e| Error::msg(format!("Invalid JSON: {e}")))?;

    let event = event_payload(&value);
    let ty = event_type(event)?;

    match ty {
        "session.idle" => {
            let session_id = resolve_session_id(event)
                .or_else(|| resolve_session_id(&value))
                .ok_or_else(|| Error::msg("sessionID is required for session.idle"))?;

            Ok(Some(OpencodeSupportedEvent::SessionIdle {
                session_id: session_id.to_string(),
            }))
        }
        "permission.updated" | "permission.asked" => {
            let properties = event
                .get("properties")
                .cloned()
                .unwrap_or_else(|| strip_event_type(event.clone()));
            let permission = parse_permission(ty, properties, event, &value)?;

            Ok(Some(OpencodeSupportedEvent::Permission {
                event_type: ty.to_string(),
                permission: Box::new(permission),
            }))
        }
        "permission.replied" => {
            let properties = event
                .get("properties")
                .cloned()
                .unwrap_or_else(|| strip_event_type(event.clone()));

            let reply = serde_json::from_value::<PermissionRepliedProperties>(properties).map_err(
                |e| Error::msg(format!("Invalid permission.replied properties: {e}")),
            )?;

            Ok(Some(OpencodeSupportedEvent::PermissionReplied {
                session_id: reply.session_id,
                request_id: reply.request_id,
                reply: reply.reply,
            }))
        }
        "session.error" => {
            let session_id = resolve_session_id(event)
                .or_else(|| resolve_session_id(&value))
                .map(|s| s.to_string());

            let error = resolve_error_value(event, &value).cloned();
            let summary = error
                .as_ref()
                .map(error_summary)
                .unwrap_or_else(|| "Session error".to_string());

            Ok(Some(OpencodeSupportedEvent::SessionError {
                session_id,
                summary,
                error,
            }))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_session_idle_flattened() {
        let evt = parse_supported_event(r#"{"type":"session.idle","sessionID":"abc123"}"#)
            .unwrap()
            .unwrap();
        assert_eq!(
            evt,
            OpencodeSupportedEvent::SessionIdle {
                session_id: "abc123".to_string()
            }
        );
    }

    #[test]
    fn parses_session_idle_canonical() {
        let evt =
            parse_supported_event(r#"{"type":"session.idle","properties":{"sessionID":"abc123"}}"#)
                .unwrap()
                .unwrap();
        assert_eq!(
            evt,
            OpencodeSupportedEvent::SessionIdle {
                session_id: "abc123".to_string()
            }
        );
    }

    #[test]
    fn parses_session_idle_wrapped_payload() {
        let evt = parse_supported_event(
            r#"{"directory":"/tmp","payload":{"type":"session.idle","properties":{"sessionID":"abc123"}}}"#,
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            evt,
            OpencodeSupportedEvent::SessionIdle {
                session_id: "abc123".to_string()
            }
        );
    }

    #[test]
    fn parses_session_idle_wrapped_event() {
        let evt =
            parse_supported_event(r#"{"event":{"type":"session.idle","sessionID":"abc123"}}"#)
                .unwrap()
                .unwrap();

        assert_eq!(
            evt,
            OpencodeSupportedEvent::SessionIdle {
                session_id: "abc123".to_string()
            }
        );
    }

    #[test]
    fn session_idle_missing_session_id_errors() {
        let err = parse_supported_event(r#"{"type":"session.idle"}"#)
            .unwrap_err()
            .to_string();
        assert!(err.contains("sessionID is required for session.idle"));
    }

    #[test]
    fn invalid_json_errors() {
        let err = parse_supported_event("not-json").unwrap_err().to_string();
        assert!(err.contains("Invalid JSON"));
    }

    #[test]
    fn parses_permission_updated_canonical() {
        let evt = parse_supported_event(
            r#"{
              "type":"permission.updated",
              "properties":{
                "id":"perm1",
                "type":"tool",
                "sessionID":"abc123",
                "messageID":"msg1",
                "title":"Allow tool?",
                "metadata":{},
                "time":{"created":1}
              }
            }"#,
        )
        .unwrap()
        .unwrap();

        match evt {
            OpencodeSupportedEvent::Permission {
                event_type,
                permission,
            } => {
                assert_eq!(event_type, "permission.updated");
                assert_eq!(permission.session_id, "abc123");
                assert_eq!(permission.title.as_deref(), Some("Allow tool?"));
            }
            _ => panic!("expected Permission"),
        }
    }

    #[test]
    fn parses_permission_asked_schema_shape() {
        let evt = parse_supported_event(
            r#"{
              "type":"permission.asked",
              "properties":{
                "id":"perm1",
                "sessionID":"abc123",
                "permission":"tool",
                "patterns":["src/**"],
                "metadata":{"k":"v"},
                "always":[],
                "tool":{"messageID":"msg1","callID":"call1"}
              }
            }"#,
        )
        .unwrap()
        .unwrap();

        match evt {
            OpencodeSupportedEvent::Permission {
                event_type,
                permission,
            } => {
                assert_eq!(event_type, "permission.asked");
                assert_eq!(permission.session_id, "abc123");
                assert_eq!(permission.permission.as_deref(), Some("tool"));
                assert_eq!(permission.patterns, vec!["src/**".to_string()]);
                assert!(permission.tool.is_some());
                assert!(permission.metadata.contains_key("k"));
            }
            _ => panic!("expected Permission"),
        }
    }

    #[test]
    fn parses_permission_replied_schema_shape() {
        let evt = parse_supported_event(
            r#"{
              "type":"permission.replied",
              "properties":{
                "sessionID":"abc123",
                "requestID":"perm1",
                "reply":"once"
              }
            }"#,
        )
        .unwrap()
        .unwrap();

        match evt {
            OpencodeSupportedEvent::PermissionReplied {
                session_id,
                request_id,
                reply,
            } => {
                assert_eq!(session_id, "abc123");
                assert_eq!(request_id, "perm1");
                assert_eq!(reply, "once");
            }
            _ => panic!("expected PermissionReplied"),
        }
    }

    #[test]
    fn unknown_event_type_is_noop() {
        let evt = parse_supported_event(r#"{"type":"something.else"}"#).unwrap();
        assert!(evt.is_none());
    }

    #[test]
    fn parses_session_error_with_object_error_canonical() {
        let evt = parse_supported_event(
            r#"{
              "type": "session.error",
              "properties": {
                "sessionID": "abc123",
                "error": { "name": "UnknownError", "data": { "message": "boom" } }
              }
            }"#,
        )
        .unwrap()
        .unwrap();

        match evt {
            OpencodeSupportedEvent::SessionError {
                session_id,
                summary,
                error,
            } => {
                assert_eq!(session_id.as_deref(), Some("abc123"));
                assert!(summary.contains("UnknownError"));
                assert!(summary.contains("boom"));
                assert!(error.is_some());
            }
            _ => panic!("expected SessionError"),
        }
    }

    #[test]
    fn parses_session_error_with_string_error_flattened() {
        let evt = parse_supported_event(
            r#"{ "type": "session.error", "sessionID": "abc123", "error": "boom" }"#,
        )
        .unwrap()
        .unwrap();

        match evt {
            OpencodeSupportedEvent::SessionError {
                session_id,
                summary,
                error,
            } => {
                assert_eq!(session_id.as_deref(), Some("abc123"));
                assert!(summary.contains("boom"));
                assert!(error.is_some());
            }
            _ => panic!("expected SessionError"),
        }
    }

    #[test]
    fn parses_session_error_without_session_id_or_error() {
        let evt = parse_supported_event(r#"{ "type": "session.error", "properties": {} }"#)
            .unwrap()
            .unwrap();

        match evt {
            OpencodeSupportedEvent::SessionError {
                session_id,
                summary,
                error,
            } => {
                assert!(session_id.is_none());
                assert_eq!(summary, "Session error");
                assert!(error.is_none());
            }
            _ => panic!("expected SessionError"),
        }
    }
}

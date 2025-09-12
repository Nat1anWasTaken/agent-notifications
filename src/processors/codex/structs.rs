use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum NotificationType {
    AgentTurnComplete,
    #[serde(other)]
    Unknown,
}

impl fmt::Display for NotificationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            NotificationType::AgentTurnComplete => "AgentTurnComplete",
            NotificationType::Unknown => "Unknown",
        };
        write!(f, "{}", name)
    }
}

impl NotificationType {
    pub fn as_str(&self) -> &str {
        match self {
            NotificationType::AgentTurnComplete => "AgentTurnComplete",
            NotificationType::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct CodexNotificationInput {
    pub r#type: NotificationType,
    #[serde(default)]
    pub turn_id: Option<String>,
    #[serde(default)]
    pub input_messages: Option<Vec<String>>,
    #[serde(default)]
    pub last_assistant_message: Option<String>,
}

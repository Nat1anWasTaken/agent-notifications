use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use strum::EnumIter;

/// Hook event names
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, EnumIter)]
#[serde(rename_all = "PascalCase")]
pub enum HookEventName {
    PreToolUse,
    PostToolUse,
    Notification,
    UserPromptSubmit,
    Stop,
    SubagentStop,
    PreCompact,
    SessionStart,
    SessionEnd,
}

impl fmt::Display for HookEventName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            HookEventName::Notification => "Notification",
            HookEventName::PreToolUse => "PreToolUse",
            HookEventName::PostToolUse => "PostToolUse",
            HookEventName::UserPromptSubmit => "UserPromptSubmit",
            HookEventName::Stop => "Stop",
            HookEventName::SubagentStop => "SubagentStop",
            HookEventName::PreCompact => "PreCompact",
            HookEventName::SessionStart => "SessionStart",
            HookEventName::SessionEnd => "SessionEnd",
        };
        write!(f, "{}", name)
    }
}

/// Trigger source for PreCompact
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PreCompactTrigger {
    /// Invoked via `/compact` command
    Manual,
    /// Invoked automatically when the context window is full
    Auto,
}

/// Source of SessionStart
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SessionStartSource {
    /// Started normally
    Startup,
    /// Resumed using `--resume`, `--continue`, or `/resume`
    Resume,
    /// Started via `/clear`
    Clear,
}

/// Reason for SessionEnd
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SessionEndReason {
    /// Session cleared with `/clear`
    Clear,
    /// User logged out
    Logout,
    /// User exited while prompt input was visible
    PromptInputExit,
    /// Other reasons
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct HookInput {
    // ---- Common fields ----
    pub session_id: String,
    pub transcript_path: String,
    #[serde(default)]
    pub cwd: Option<String>,
    pub hook_event_name: HookEventName,

    // ---- PreToolUse / PostToolUse specific ----
    /// Tool name (e.g., Write, Edit, Bash, or mcp__...)
    #[serde(default)]
    pub tool_name: Option<String>,
    /// Tool input: schema varies depending on the tool
    #[serde(default)]
    pub tool_input: Option<Value>,
    /// Tool response: schema varies depending on the tool, only present in PostToolUse
    #[serde(default)]
    pub tool_response: Option<Value>,

    // ---- Notification specific ----
    #[serde(default)]
    pub message: Option<String>,

    // ---- UserPromptSubmit specific ----
    #[serde(default)]
    pub prompt: Option<String>,

    // ---- Stop / SubagentStop specific ----
    /// True if Claude Code is already continuing because of a Stop hook
    #[serde(default)]
    pub stop_hook_active: Option<bool>,

    // ---- PreCompact specific ----
    #[serde(default)]
    pub trigger: Option<PreCompactTrigger>,
    /// User input for manual compaction; empty string for auto compaction
    #[serde(default)]
    pub custom_instructions: Option<String>,

    // ---- SessionStart specific ----
    #[serde(default)]
    pub source: Option<SessionStartSource>,

    // ---- SessionEnd specific ----
    #[serde(default)]
    pub reason: Option<SessionEndReason>,
}

/// The overall JSON structure that a hook script can output to Claude Code.
/// All fields are optional because scripts can choose what to include.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HookOutput {
    /// Whether Claude should continue after hook execution (default: true)
    #[serde(default)]
    pub r#continue: Option<bool>,

    /// Message shown to the user when `continue` is false
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,

    /// Whether to hide stdout from transcript mode (default: false)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suppress_output: Option<bool>,

    /// Optional warning or info message shown to the user
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_message: Option<String>,

    /// Decision control: varies by hook type
    /// - `block` or `undefined` for most hooks
    /// - For PreToolUse, use `hookSpecificOutput.permissionDecision`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<String>,

    /// Explanation for the decision (shown either to user or Claude)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,

    /// Hook-specific nested output, depends on event type
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hook_specific_output: Option<HookSpecificOutput>,
}

impl Default for HookOutput {
    fn default() -> Self {
        Self {
            r#continue: Some(true),
            stop_reason: None,
            suppress_output: Some(false),
            system_message: None,
            decision: None,
            reason: None,
            hook_specific_output: None,
        }
    }
}

/// Nested object for event-specific control
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HookSpecificOutput {
    /// The hook event this output applies to
    #[serde(default)]
    pub hook_event_name: Option<String>,

    /// Adds context for Claude to consider
    #[serde(default)]
    pub additional_context: Option<String>,

    /// PreToolUse-specific permission control
    #[serde(default)]
    pub permission_decision: Option<PermissionDecision>,

    /// Reason for permission decision
    #[serde(default)]
    pub permission_decision_reason: Option<String>,
}

/// PreToolUse permission decision types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PermissionDecision {
    /// Allow without asking the user
    Allow,
    /// Deny tool execution
    Deny,
    /// Ask the user for confirmation
    Ask,
}

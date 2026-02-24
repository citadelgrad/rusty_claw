//! Hook input and context types.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Typed hook input variants for the 10 lifecycle events.
///
/// Each variant carries only the fields relevant to that specific event,
/// providing compile-time safety instead of the previous stringly-typed
/// `HookInput` struct with optional fields.
///
/// # Examples
///
/// ```
/// use rusty_claw::hooks::HookEventInput;
/// use serde_json::json;
///
/// let input = HookEventInput::PreToolUse {
///     tool_name: "Bash".to_string(),
///     tool_input: json!({"command": "ls"}),
/// };
///
/// match &input {
///     HookEventInput::PreToolUse { tool_name, .. } => {
///         assert_eq!(tool_name, "Bash");
///     }
///     _ => {}
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "hook_event_name", rename_all = "PascalCase")]
pub enum HookEventInput {
    /// Fired before a tool is invoked.
    ///
    /// Available in: `PreToolUse` hooks.
    PreToolUse {
        /// Name of the tool about to be invoked.
        tool_name: String,
        /// JSON input parameters for the tool.
        tool_input: Value,
    },

    /// Fired after a tool completes successfully.
    ///
    /// Available in: `PostToolUse` hooks.
    PostToolUse {
        /// Name of the tool that was invoked.
        tool_name: String,
        /// JSON input parameters that were passed to the tool.
        tool_input: Value,
        /// JSON response/output from the tool.
        tool_response: Value,
    },

    /// Fired after a tool fails with an error.
    ///
    /// Available in: `PostToolUseFailure` hooks.
    PostToolUseFailure {
        /// Name of the tool that failed.
        tool_name: String,
        /// JSON input parameters that were passed to the tool.
        tool_input: Value,
        /// Error message describing the failure.
        error: String,
    },

    /// Fired when the user submits a new prompt.
    ///
    /// Available in: `UserPromptSubmit` hooks.
    UserPromptSubmit {
        /// The prompt text submitted by the user.
        prompt: String,
        /// Optional additional context for the prompt.
        #[serde(skip_serializing_if = "Option::is_none")]
        context: Option<HashMap<String, Value>>,
    },

    /// Fired when the agent session stops.
    ///
    /// Available in: `Stop` hooks.
    Stop {
        /// Whether a stop hook is already active (prevents re-entrancy).
        stop_hook_active: bool,
        /// Path to the session transcript file, if available.
        #[serde(skip_serializing_if = "Option::is_none")]
        transcript_path: Option<String>,
    },

    /// Fired when a subagent session stops.
    ///
    /// Available in: `SubagentStop` hooks.
    SubagentStop {
        /// Identifier for the subagent.
        agent_id: String,
        /// Type/role of the subagent.
        agent_type: String,
        /// Path to the subagent's transcript file, if available.
        #[serde(skip_serializing_if = "Option::is_none")]
        transcript_path: Option<String>,
    },

    /// Fired when a subagent session starts.
    ///
    /// Available in: `SubagentStart` hooks.
    SubagentStart {
        /// Identifier for the subagent.
        agent_id: String,
        /// Type/role of the subagent.
        agent_type: String,
    },

    /// Fired before conversation compaction.
    ///
    /// Available in: `PreCompact` hooks.
    PreCompact {
        /// What triggered the compaction (e.g., "auto", "manual").
        trigger: String,
        /// Custom instructions for the compaction summary.
        #[serde(skip_serializing_if = "Option::is_none")]
        custom_instructions: Option<String>,
        /// Existing conversation summary, if any.
        #[serde(skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
    },

    /// Fired for system notification events.
    ///
    /// Available in: `Notification` hooks.
    Notification {
        /// Title of the notification.
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        /// Notification message body.
        message: String,
    },

    /// Fired when Claude requests permission to use a tool.
    ///
    /// Available in: `PermissionRequest` hooks.
    PermissionRequest {
        /// Name of the tool for which permission is being requested.
        tool_name: String,
        /// JSON input parameters for the tool.
        tool_input: Value,
        /// Permission suggestions from the CLI.
        #[serde(skip_serializing_if = "Option::is_none")]
        permission_suggestions: Option<Vec<Value>>,
        /// Path that would be accessed, if applicable.
        #[serde(skip_serializing_if = "Option::is_none")]
        blocked_path: Option<String>,
    },
}

impl HookEventInput {
    /// Returns the hook event name for this input variant.
    pub fn event_name(&self) -> &'static str {
        match self {
            HookEventInput::PreToolUse { .. } => "PreToolUse",
            HookEventInput::PostToolUse { .. } => "PostToolUse",
            HookEventInput::PostToolUseFailure { .. } => "PostToolUseFailure",
            HookEventInput::UserPromptSubmit { .. } => "UserPromptSubmit",
            HookEventInput::Stop { .. } => "Stop",
            HookEventInput::SubagentStop { .. } => "SubagentStop",
            HookEventInput::SubagentStart { .. } => "SubagentStart",
            HookEventInput::PreCompact { .. } => "PreCompact",
            HookEventInput::Notification { .. } => "Notification",
            HookEventInput::PermissionRequest { .. } => "PermissionRequest",
        }
    }

    /// Returns the tool name if this is a tool-related event.
    pub fn tool_name(&self) -> Option<&str> {
        match self {
            HookEventInput::PreToolUse { tool_name, .. }
            | HookEventInput::PostToolUse { tool_name, .. }
            | HookEventInput::PostToolUseFailure { tool_name, .. }
            | HookEventInput::PermissionRequest { tool_name, .. } => Some(tool_name.as_str()),
            _ => None,
        }
    }
}

/// Legacy generic input data passed to a hook callback.
///
/// For new code, prefer [`HookEventInput`] which provides typed, per-event
/// variants with compile-time field guarantees.
///
/// # Examples
///
/// ```
/// use rusty_claw::prelude::*;
/// use serde_json::json;
///
/// let input = HookInput {
///     tool_name: Some("Bash".to_string()),
///     tool_input: Some(json!({"command": "ls"})),
///     ..Default::default()
/// };
///
/// assert_eq!(input.tool_name.as_deref(), Some("Bash"));
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookInput {
    /// Name of the tool being invoked (for tool-related events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,

    /// Input parameters for the tool (for tool-related events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_input: Option<Value>,

    /// Output from the tool (for post-tool events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_output: Option<Value>,

    /// Error message (for failure events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// User prompt text (for UserPromptSubmit events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,

    /// Additional event-specific data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, Value>>,
}

impl HookInput {
    /// Create a new HookInput for a tool use event
    pub fn tool_use(tool_name: impl Into<String>, tool_input: Value) -> Self {
        Self {
            tool_name: Some(tool_name.into()),
            tool_input: Some(tool_input),
            ..Default::default()
        }
    }

    /// Create a new HookInput for a tool success event
    pub fn tool_success(tool_name: impl Into<String>, output: Value) -> Self {
        Self {
            tool_name: Some(tool_name.into()),
            tool_output: Some(output),
            ..Default::default()
        }
    }

    /// Create a new HookInput for a tool failure event
    pub fn tool_failure(tool_name: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            tool_name: Some(tool_name.into()),
            error: Some(error.into()),
            ..Default::default()
        }
    }

    /// Create a new HookInput for a user prompt event
    pub fn prompt(text: impl Into<String>) -> Self {
        Self {
            prompt: Some(text.into()),
            ..Default::default()
        }
    }
}

/// Convert from typed [`HookEventInput`] to legacy [`HookInput`] for backward compatibility.
impl From<HookEventInput> for HookInput {
    fn from(typed: HookEventInput) -> Self {
        match typed {
            HookEventInput::PreToolUse { tool_name, tool_input } => HookInput {
                tool_name: Some(tool_name),
                tool_input: Some(tool_input),
                ..Default::default()
            },
            HookEventInput::PostToolUse { tool_name, tool_input, tool_response } => HookInput {
                tool_name: Some(tool_name),
                tool_input: Some(tool_input),
                tool_output: Some(tool_response),
                ..Default::default()
            },
            HookEventInput::PostToolUseFailure { tool_name, tool_input, error } => HookInput {
                tool_name: Some(tool_name),
                tool_input: Some(tool_input),
                error: Some(error),
                ..Default::default()
            },
            HookEventInput::UserPromptSubmit { prompt, .. } => HookInput {
                prompt: Some(prompt),
                ..Default::default()
            },
            HookEventInput::Stop { .. }
            | HookEventInput::SubagentStop { .. }
            | HookEventInput::SubagentStart { .. }
            | HookEventInput::PreCompact { .. } => HookInput::default(),
            HookEventInput::Notification { message, .. } => HookInput {
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("message".to_string(), Value::String(message));
                    Some(m)
                },
                ..Default::default()
            },
            HookEventInput::PermissionRequest { tool_name, tool_input, .. } => HookInput {
                tool_name: Some(tool_name),
                tool_input: Some(tool_input),
                ..Default::default()
            },
        }
    }
}

/// Context provided to hook callbacks
///
/// # Examples
///
/// ```
/// use rusty_claw::prelude::*;
///
/// let context = HookContext {
///     session_id: Some("session-123".to_string()),
///     ..Default::default()
/// };
///
/// assert_eq!(context.session_id.as_deref(), Some("session-123"));
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookContext {
    /// Current session ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,

    /// Available tools in the session
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available_tools: Option<Vec<String>>,

    /// Active subagents
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agents: Option<Vec<String>>,

    /// MCP servers connected
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<Vec<String>>,

    /// Additional context data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, Value>>,
}

impl HookContext {
    /// Create a new HookContext with a session ID
    pub fn with_session(session_id: impl Into<String>) -> Self {
        Self {
            session_id: Some(session_id.into()),
            ..Default::default()
        }
    }

    /// Set available tools
    pub fn with_tools(mut self, tools: Vec<String>) -> Self {
        self.available_tools = Some(tools);
        self
    }

    /// Set active agents
    pub fn with_agents(mut self, agents: Vec<String>) -> Self {
        self.agents = Some(agents);
        self
    }

    /// Set MCP servers
    pub fn with_mcp_servers(mut self, servers: Vec<String>) -> Self {
        self.mcp_servers = Some(servers);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_hook_event_input_pre_tool_use() {
        let input = HookEventInput::PreToolUse {
            tool_name: "Bash".to_string(),
            tool_input: json!({"command": "ls"}),
        };
        assert_eq!(input.event_name(), "PreToolUse");
        assert_eq!(input.tool_name(), Some("Bash"));
    }

    #[test]
    fn test_hook_event_input_post_tool_use() {
        let input = HookEventInput::PostToolUse {
            tool_name: "Read".to_string(),
            tool_input: json!({"path": "/tmp/test"}),
            tool_response: json!({"content": "file content"}),
        };
        assert_eq!(input.event_name(), "PostToolUse");
        assert_eq!(input.tool_name(), Some("Read"));
    }

    #[test]
    fn test_hook_event_input_post_tool_use_failure() {
        let input = HookEventInput::PostToolUseFailure {
            tool_name: "Bash".to_string(),
            tool_input: json!({"command": "bad_cmd"}),
            error: "command not found".to_string(),
        };
        assert_eq!(input.event_name(), "PostToolUseFailure");
        assert_eq!(input.tool_name(), Some("Bash"));
    }

    #[test]
    fn test_hook_event_input_user_prompt_submit() {
        let input = HookEventInput::UserPromptSubmit {
            prompt: "Hello, Claude!".to_string(),
            context: None,
        };
        assert_eq!(input.event_name(), "UserPromptSubmit");
        assert_eq!(input.tool_name(), None);
    }

    #[test]
    fn test_hook_event_input_stop() {
        let input = HookEventInput::Stop {
            stop_hook_active: false,
            transcript_path: Some("/tmp/transcript.json".to_string()),
        };
        assert_eq!(input.event_name(), "Stop");
        assert_eq!(input.tool_name(), None);
    }

    #[test]
    fn test_hook_event_input_subagent_stop() {
        let input = HookEventInput::SubagentStop {
            agent_id: "agent-1".to_string(),
            agent_type: "worker".to_string(),
            transcript_path: None,
        };
        assert_eq!(input.event_name(), "SubagentStop");
    }

    #[test]
    fn test_hook_event_input_subagent_start() {
        let input = HookEventInput::SubagentStart {
            agent_id: "agent-2".to_string(),
            agent_type: "subagent".to_string(),
        };
        assert_eq!(input.event_name(), "SubagentStart");
    }

    #[test]
    fn test_hook_event_input_pre_compact() {
        let input = HookEventInput::PreCompact {
            trigger: "auto".to_string(),
            custom_instructions: None,
            summary: Some("Previous conversation summary".to_string()),
        };
        assert_eq!(input.event_name(), "PreCompact");
    }

    #[test]
    fn test_hook_event_input_notification() {
        let input = HookEventInput::Notification {
            title: Some("Task Complete".to_string()),
            message: "Your task has finished.".to_string(),
        };
        assert_eq!(input.event_name(), "Notification");
        assert_eq!(input.tool_name(), None);
    }

    #[test]
    fn test_hook_event_input_permission_request() {
        let input = HookEventInput::PermissionRequest {
            tool_name: "Write".to_string(),
            tool_input: json!({"path": "/etc/passwd"}),
            permission_suggestions: None,
            blocked_path: Some("/etc".to_string()),
        };
        assert_eq!(input.event_name(), "PermissionRequest");
        assert_eq!(input.tool_name(), Some("Write"));
    }

    #[test]
    fn test_hook_event_input_serialization() {
        let input = HookEventInput::PreToolUse {
            tool_name: "Bash".to_string(),
            tool_input: json!({"command": "ls"}),
        };
        let json = serde_json::to_value(&input).unwrap();
        assert_eq!(json["hook_event_name"], "PreToolUse");
        assert_eq!(json["tool_name"], "Bash");
        assert_eq!(json["tool_input"]["command"], "ls");
    }

    #[test]
    fn test_hook_event_input_from_legacy() {
        // Test From conversion
        let typed = HookEventInput::PreToolUse {
            tool_name: "Bash".to_string(),
            tool_input: json!({"command": "ls"}),
        };
        let legacy: HookInput = typed.into();
        assert_eq!(legacy.tool_name.as_deref(), Some("Bash"));
        assert!(legacy.tool_input.is_some());
    }

    #[test]
    fn test_hook_event_input_post_tool_use_from_legacy() {
        let typed = HookEventInput::PostToolUse {
            tool_name: "Read".to_string(),
            tool_input: json!({"path": "/tmp/x"}),
            tool_response: json!({"content": "data"}),
        };
        let legacy: HookInput = typed.into();
        assert_eq!(legacy.tool_name.as_deref(), Some("Read"));
        assert!(legacy.tool_output.is_some());
    }

    #[test]
    fn test_hook_input_tool_use() {
        let input = HookInput::tool_use("Bash", json!({"command": "ls"}));
        assert_eq!(input.tool_name.as_deref(), Some("Bash"));
        assert!(input.tool_input.is_some());
    }

    #[test]
    fn test_hook_input_tool_success() {
        let input = HookInput::tool_success("Bash", json!({"output": "file.txt"}));
        assert_eq!(input.tool_name.as_deref(), Some("Bash"));
        assert!(input.tool_output.is_some());
    }

    #[test]
    fn test_hook_input_tool_failure() {
        let input = HookInput::tool_failure("Bash", "Command failed");
        assert_eq!(input.tool_name.as_deref(), Some("Bash"));
        assert_eq!(input.error.as_deref(), Some("Command failed"));
    }

    #[test]
    fn test_hook_input_prompt() {
        let input = HookInput::prompt("Hello, Claude!");
        assert_eq!(input.prompt.as_deref(), Some("Hello, Claude!"));
    }

    #[test]
    fn test_hook_input_serialization() {
        let input = HookInput::tool_use("Bash", json!({"command": "ls"}));
        let json = serde_json::to_value(&input).unwrap();

        assert_eq!(json["tool_name"], "Bash");
        assert_eq!(json["tool_input"]["command"], "ls");
    }

    #[test]
    fn test_hook_context_builder() {
        let context = HookContext::with_session("session-123")
            .with_tools(vec!["Bash".to_string(), "Read".to_string()])
            .with_agents(vec!["agent1".to_string()])
            .with_mcp_servers(vec!["server1".to_string()]);

        assert_eq!(context.session_id.as_deref(), Some("session-123"));
        assert_eq!(context.available_tools.as_ref().unwrap().len(), 2);
        assert_eq!(context.agents.as_ref().unwrap().len(), 1);
        assert_eq!(context.mcp_servers.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_hook_context_serialization() {
        let context = HookContext::with_session("session-123");
        let json = serde_json::to_value(&context).unwrap();

        assert_eq!(json["session_id"], "session-123");
    }
}

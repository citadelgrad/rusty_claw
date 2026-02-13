//! Hook input and context types.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Input data passed to a hook callback
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

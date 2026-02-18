//! Control protocol message types for bidirectional communication with Claude CLI
//!
//! This module defines the message types used in the control protocol:
//! - [`ControlRequest`] - Outgoing requests from SDK to CLI
//! - [`ControlResponse`] - Responses to control requests
//! - [`IncomingControlRequest`] - Incoming requests from CLI to SDK
//!
//! # Message Flow
//!
//! **SDK → CLI (Outgoing):**
//! ```text
//! {
//!   "type": "control_request",
//!   "request_id": "uuid",
//!   "request": { "subtype": "initialize", ... }
//! }
//! ```
//!
//! **CLI → SDK (Response):**
//! ```text
//! {
//!   "type": "control_response",
//!   "request_id": "uuid",
//!   "response": { "subtype": "success", ... }
//! }
//! ```
//!
//! **CLI → SDK (Incoming Request):**
//! ```text
//! {
//!   "type": "control_request",
//!   "request_id": "uuid",
//!   "request": { "subtype": "can_use_tool", ... }
//! }
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::options::{AgentDefinition, HookEvent, HookMatcher};

/// Outgoing control requests from SDK to Claude CLI
///
/// These messages are sent from the SDK to control CLI behavior during a session.
/// All requests follow the format:
/// ```json
/// {
///   "type": "control_request",
///   "request_id": "uuid",
///   "request": { "subtype": "...", ... }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum ControlRequest {
    /// Initialize the agent session with hooks, agents, and MCP servers
    ///
    /// Must be sent before any other interaction. Configures:
    /// - Hook callbacks for events (tool use, messages, etc.)
    /// - Agent definitions for spawning subagents
    /// - SDK-hosted MCP servers for tool execution
    ///
    /// Note: Permission mode is set via the `--permission-mode` CLI flag,
    /// not in the initialize request.
    ///
    /// # Example
    /// ```json
    /// {
    ///   "subtype": "initialize",
    ///   "hooks": {},
    ///   "agents": {},
    ///   "sdk_mcp_servers": []
    /// }
    /// ```
    Initialize {
        /// Hook event matchers for callback registration
        #[serde(skip_serializing_if = "HashMap::is_empty", default)]
        hooks: HashMap<HookEvent, Vec<HookMatcher>>,

        /// Agent definitions for spawning subagents
        #[serde(skip_serializing_if = "HashMap::is_empty", default)]
        agents: HashMap<String, AgentDefinition>,

        /// SDK-hosted MCP server names (strings, not objects)
        ///
        /// The CLI expects `sdkMcpServers: ["name1", "name2"]` — just names.
        #[serde(
            rename = "sdkMcpServers",
            skip_serializing_if = "Vec::is_empty",
            default
        )]
        sdk_mcp_servers: Vec<String>,
    },

    /// Interrupt the current agent execution
    ///
    /// Sends a cancellation signal to stop ongoing processing.
    /// The CLI will finish the current operation and return control.
    Interrupt,

    /// Change the permission mode during execution
    ///
    /// Dynamically adjusts how tool permissions are handled.
    ///
    /// # Example
    /// ```json
    /// {
    ///   "subtype": "set_permission_mode",
    ///   "mode": "accept_edits"
    /// }
    /// ```
    SetPermissionMode {
        /// New permission mode (e.g., "default", "accept_edits", "bypass_permissions")
        mode: String,
    },

    /// Switch the active model during execution
    ///
    /// Changes which Claude model processes subsequent turns.
    ///
    /// # Example
    /// ```json
    /// {
    ///   "subtype": "set_model",
    ///   "model": "claude-sonnet-4"
    /// }
    /// ```
    SetModel {
        /// Model identifier (e.g., "claude-sonnet-4", "claude-opus-4")
        model: String,
    },

    /// Query MCP server connection status
    ///
    /// Returns information about connected MCP servers and their tools.
    McpStatus,

    /// Rewind file state to a specific message
    ///
    /// Rolls back filesystem changes to the state at the given message ID.
    ///
    /// # Example
    /// ```json
    /// {
    ///   "subtype": "rewind_files",
    ///   "message_id": "msg_123"
    /// }
    /// ```
    RewindFiles {
        /// Message ID to rewind to
        message_id: String,
    },
}

/// Response to a control request
///
/// Sent from CLI → SDK or SDK → CLI in response to control requests.
/// All responses include a `subtype` of either "success" or "error".
///
/// # Success Example
/// ```json
/// {
///   "subtype": "success",
///   "data": { "foo": "bar" }
/// }
/// ```
///
/// # Error Example
/// ```json
/// {
///   "subtype": "error",
///   "error": "Tool not found",
///   "code": "tool_not_found"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum ControlResponse {
    /// Successful control request response
    Success {
        /// Response data (structure depends on request type)
        #[serde(flatten)]
        data: Value,
    },

    /// Error response
    Error {
        /// Human-readable error message
        error: String,

        /// Additional error context (error codes, details, etc.)
        #[serde(flatten)]
        extra: Value,
    },
}

/// Incoming control requests from Claude CLI to SDK
///
/// These messages are sent from the CLI to request SDK actions or permissions.
/// The SDK must respond with a [`ControlResponse`].
///
/// # Message Format
/// ```json
/// {
///   "type": "control_request",
///   "request_id": "uuid",
///   "request": { "subtype": "...", ... }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum IncomingControlRequest {
    /// Request permission to use a tool
    ///
    /// The CLI asks the SDK whether a specific tool invocation should be allowed.
    /// The SDK should respond with `Success { data: { "allowed": true/false } }`.
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "subtype": "can_use_tool",
    ///   "tool_name": "Bash",
    ///   "tool_input": { "command": "ls -la" }
    /// }
    /// ```
    ///
    /// # Example Response
    /// ```json
    /// {
    ///   "subtype": "success",
    ///   "allowed": true
    /// }
    /// ```
    CanUseTool {
        /// Name of the tool to be used
        tool_name: String,

        /// Tool input parameters
        tool_input: Value,
    },

    /// Invoke a registered hook callback
    ///
    /// The CLI triggers a hook based on registered matchers.
    /// The SDK should execute the hook and return the result.
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "subtype": "hook_callback",
    ///   "hook_id": "pre_commit_hook",
    ///   "hook_event": "tool_use",
    ///   "hook_input": { "tool": "Bash", "command": "git commit" }
    /// }
    /// ```
    ///
    /// # Example Response
    /// ```json
    /// {
    ///   "subtype": "success",
    ///   "output": "Hook executed successfully"
    /// }
    /// ```
    HookCallback {
        /// Unique hook identifier
        hook_id: String,

        /// Event that triggered the hook
        hook_event: HookEvent,

        /// Hook input data
        hook_input: Value,
    },

    /// Route an MCP message to an SDK-hosted server
    ///
    /// The CLI forwards a JSON-RPC message to an MCP server hosted by the SDK.
    /// The SDK should route to the appropriate server and return the JSON-RPC response.
    ///
    /// # Example Request
    /// ```json
    /// {
    ///   "subtype": "mcp_message",
    ///   "server_name": "my_tool_server",
    ///   "message": {
    ///     "jsonrpc": "2.0",
    ///     "id": 1,
    ///     "method": "tools/call",
    ///     "params": { "name": "my_tool", "arguments": {} }
    ///   }
    /// }
    /// ```
    ///
    /// # Example Response
    /// ```json
    /// {
    ///   "subtype": "success",
    ///   "jsonrpc": "2.0",
    ///   "id": 1,
    ///   "result": { "content": [{ "type": "text", "text": "Done" }] }
    /// }
    /// ```
    McpMessage {
        /// Name of the SDK-hosted MCP server
        server_name: String,

        /// JSON-RPC message to forward
        message: Value,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_control_request_initialize_minimal() {
        let req = ControlRequest::Initialize {
            hooks: HashMap::new(),
            agents: HashMap::new(),
            sdk_mcp_servers: vec![],
        };

        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["subtype"], "initialize");
        // Empty collections should be omitted
        assert!(json.get("hooks").is_none());
        assert!(json.get("agents").is_none());
        assert!(json.get("sdkMcpServers").is_none());
    }

    #[test]
    fn test_control_request_initialize_roundtrip() {
        let req = ControlRequest::Initialize {
            hooks: HashMap::new(),
            agents: HashMap::new(),
            sdk_mcp_servers: vec![],
        };

        let json = serde_json::to_string(&req).unwrap();
        let parsed: ControlRequest = serde_json::from_str(&json).unwrap();

        match parsed {
            ControlRequest::Initialize {
                hooks,
                agents,
                sdk_mcp_servers,
            } => {
                assert!(hooks.is_empty());
                assert!(agents.is_empty());
                assert!(sdk_mcp_servers.is_empty());
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_control_request_interrupt() {
        let req = ControlRequest::Interrupt;
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["subtype"], "interrupt");
    }

    #[test]
    fn test_control_request_set_permission_mode() {
        let req = ControlRequest::SetPermissionMode {
            mode: "accept_edits".to_string(),
        };

        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["subtype"], "set_permission_mode");
        assert_eq!(json["mode"], "accept_edits");
    }

    #[test]
    fn test_control_request_set_model() {
        let req = ControlRequest::SetModel {
            model: "claude-sonnet-4".to_string(),
        };

        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["subtype"], "set_model");
        assert_eq!(json["model"], "claude-sonnet-4");
    }

    #[test]
    fn test_control_request_mcp_status() {
        let req = ControlRequest::McpStatus;
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["subtype"], "mcp_status");
    }

    #[test]
    fn test_control_request_rewind_files() {
        let req = ControlRequest::RewindFiles {
            message_id: "msg_123".to_string(),
        };

        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["subtype"], "rewind_files");
        assert_eq!(json["message_id"], "msg_123");
    }

    #[test]
    fn test_control_response_success() {
        let resp = ControlResponse::Success {
            data: json!({ "allowed": true }),
        };

        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["subtype"], "success");
        assert_eq!(json["allowed"], true);
    }

    #[test]
    fn test_control_response_error() {
        let resp = ControlResponse::Error {
            error: "Tool not found".to_string(),
            extra: json!({ "code": "tool_not_found" }),
        };

        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["subtype"], "error");
        assert_eq!(json["error"], "Tool not found");
        assert_eq!(json["code"], "tool_not_found");
    }

    #[test]
    fn test_control_response_roundtrip() {
        let resp = ControlResponse::Success {
            data: json!({ "foo": "bar", "count": 42 }),
        };

        let json = serde_json::to_string(&resp).unwrap();
        let parsed: ControlResponse = serde_json::from_str(&json).unwrap();

        match parsed {
            ControlResponse::Success { data } => {
                assert_eq!(data["foo"], "bar");
                assert_eq!(data["count"], 42);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_incoming_control_request_can_use_tool() {
        let req = IncomingControlRequest::CanUseTool {
            tool_name: "Bash".to_string(),
            tool_input: json!({ "command": "ls -la" }),
        };

        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["subtype"], "can_use_tool");
        assert_eq!(json["tool_name"], "Bash");
        assert_eq!(json["tool_input"]["command"], "ls -la");
    }

    #[test]
    fn test_incoming_control_request_hook_callback() {
        let req = IncomingControlRequest::HookCallback {
            hook_id: "pre_commit".to_string(),
            hook_event: crate::options::HookEvent::PreToolUse,
            hook_input: json!({ "tool": "Bash" }),
        };

        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["subtype"], "hook_callback");
        assert_eq!(json["hook_id"], "pre_commit");
    }

    #[test]
    fn test_incoming_control_request_mcp_message() {
        let req = IncomingControlRequest::McpMessage {
            server_name: "my_server".to_string(),
            message: json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call"
            }),
        };

        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["subtype"], "mcp_message");
        assert_eq!(json["server_name"], "my_server");
        assert_eq!(json["message"]["method"], "tools/call");
    }

    #[test]
    fn test_incoming_control_request_roundtrip() {
        let req = IncomingControlRequest::CanUseTool {
            tool_name: "Read".to_string(),
            tool_input: json!({ "file_path": "/tmp/test.txt" }),
        };

        let json = serde_json::to_string(&req).unwrap();
        let parsed: IncomingControlRequest = serde_json::from_str(&json).unwrap();

        match parsed {
            IncomingControlRequest::CanUseTool {
                tool_name,
                tool_input,
            } => {
                assert_eq!(tool_name, "Read");
                assert_eq!(tool_input["file_path"], "/tmp/test.txt");
            }
            _ => panic!("Wrong variant"),
        }
    }
}

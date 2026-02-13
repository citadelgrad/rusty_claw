//! Message types and structures for Claude Code CLI communication
//!
//! This module provides types for parsing and handling messages from the Claude Code CLI.
//! All types use serde for serialization/deserialization with tagged enum variants.
//!
//! # Message Types
//!
//! The primary [`Message`] enum represents all possible messages from the CLI:
//! - [`Message::System`] - System lifecycle events (init, compact boundary)
//! - [`Message::Assistant`] - Assistant responses with content blocks
//! - [`Message::User`] - User input messages
//! - [`Message::Result`] - Final results (success, error, input required)
//!
//! # Content Blocks
//!
//! Assistant messages contain [`ContentBlock`] items:
//! - [`ContentBlock::Text`] - Plain text content
//! - [`ContentBlock::ToolUse`] - Tool invocation requests
//! - [`ContentBlock::ToolResult`] - Tool execution results
//! - [`ContentBlock::Thinking`] - Extended thinking tokens
//!
//! # Test Fixtures
//!
//! This module includes NDJSON test fixtures in `tests/fixtures/` that represent
//! realistic message sequences from the Claude Code CLI. These fixtures are used
//! for testing message deserialization and can be used in custom tests:
//!
//! - `simple_query.ndjson` - Basic query/response exchange with system init
//! - `tool_use.ndjson` - Complete tool invocation cycle with ToolUse and ToolResult
//! - `error_response.ndjson` - Error result handling scenario
//! - `thinking_content.ndjson` - Extended thinking tokens with ContentBlock::Thinking
//!
//! ## Loading Fixtures in Tests
//!
//! ```no_run
//! use std::fs::File;
//! use std::io::{BufRead, BufReader};
//! use rusty_claw::messages::Message;
//!
//! let fixture_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/simple_query.ndjson");
//! let file = File::open(fixture_path).unwrap();
//! let messages: Vec<Message> = BufReader::new(file)
//!     .lines()
//!     .map(|line| serde_json::from_str(&line.unwrap()).unwrap())
//!     .collect();
//! ```
//!
//! See SPEC.md section 10.3 for the complete message format specification.
//!
//! # Example
//!
//! ```
//! use rusty_claw::messages::{Message, ContentBlock};
//! use serde_json;
//!
//! let json = r#"{"type": "assistant", "message": {"role": "assistant", "content": [{"type": "text", "text": "Hello!"}]}}"#;
//! let msg: Message = serde_json::from_str(json).unwrap();
//! match msg {
//!     Message::Assistant(assistant_msg) => {
//!         assert_eq!(assistant_msg.message.role, "assistant");
//!     },
//!     _ => panic!("Expected assistant message"),
//! }
//! ```

use serde::{Deserialize, Serialize};

use crate::control::messages::{ControlRequest, ControlResponse};

/// Top-level message type discriminated by `type` field
///
/// All messages from Claude Code CLI are wrapped in this enum.
/// The `type` field is used for JSON deserialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    /// System lifecycle events (init, compact boundary)
    System(SystemMessage),
    /// Assistant responses with content blocks
    Assistant(AssistantMessage),
    /// User input messages
    User(UserMessage),
    /// Final results (success, error, input required)
    Result(ResultMessage),
    /// Control request (bidirectional: SDK ↔ CLI)
    ControlRequest {
        /// Unique request identifier
        request_id: String,
        /// Control request payload
        #[serde(flatten)]
        request: ControlRequest,
    },
    /// Control response (bidirectional: SDK ↔ CLI)
    ControlResponse {
        /// Request identifier this response corresponds to
        request_id: String,
        /// Control response payload
        #[serde(flatten)]
        response: ControlResponse,
    },
}

/// System message variants discriminated by `subtype` field
///
/// System messages represent lifecycle events in the agent session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum SystemMessage {
    /// Session initialization with available tools and MCP servers
    Init {
        /// Unique session identifier
        session_id: String,
        /// Available tool definitions
        tools: Vec<ToolInfo>,
        /// Connected MCP server information
        mcp_servers: Vec<McpServerInfo>,
        /// Additional fields from the CLI
        #[serde(flatten)]
        extra: serde_json::Value,
    },
    /// Marker for conversation compaction boundary
    CompactBoundary,
}

/// Assistant message containing API response with content blocks
///
/// Represents a response from Claude with text, tool use, or thinking content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessage {
    /// The API message with role and content blocks
    pub message: ApiMessage,
    /// Optional parent tool use ID if this is a nested agent response
    #[serde(default)]
    pub parent_tool_use_id: Option<String>,
    /// Optional duration of the API request in milliseconds
    #[serde(default)]
    pub duration_ms: Option<u64>,
}

/// User input message
///
/// Represents user-provided input to the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessage {
    /// The API message with role and content
    pub message: ApiMessage,
}

/// Result message variants discriminated by `subtype` field
///
/// Final outcomes of agent execution: success, error, or input needed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum ResultMessage {
    /// Successful execution with final result
    Success {
        /// The final result text
        result: String,
        /// Optional execution duration in milliseconds
        #[serde(default)]
        duration_ms: Option<u64>,
        /// Optional number of conversation turns
        #[serde(default)]
        num_turns: Option<u32>,
        /// Optional session identifier
        #[serde(default)]
        session_id: Option<String>,
        /// Optional total cost in USD
        #[serde(default)]
        total_cost_usd: Option<f64>,
        /// Optional token usage information
        #[serde(default)]
        usage: Option<UsageInfo>,
    },
    /// Error during execution
    Error {
        /// Error message text
        error: String,
        /// Additional error fields
        #[serde(flatten)]
        extra: serde_json::Value,
    },
    /// Agent requires additional user input
    InputRequired,
}

/// Content block variants discriminated by `type` field
///
/// Represents different types of content in assistant messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Plain text content
    Text {
        /// The text content
        text: String,
    },
    /// Tool invocation request
    ToolUse {
        /// Unique identifier for this tool use
        id: String,
        /// Name of the tool to invoke
        name: String,
        /// Tool input parameters as JSON
        input: serde_json::Value,
    },
    /// Tool execution result
    ToolResult {
        /// ID of the tool use this result corresponds to
        tool_use_id: String,
        /// Result data as JSON
        content: serde_json::Value,
        /// Whether this result represents an error
        #[serde(default)]
        is_error: bool,
    },
    /// Extended thinking content
    Thinking {
        /// The thinking content text
        thinking: String,
    },
}

/// Streaming event from CLI
///
/// Used for real-time updates during agent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    /// Event type identifier
    pub event_type: String,
    /// Event data payload
    pub data: serde_json::Value,
}

/// Message in Anthropic Messages API format
///
/// Standard structure for assistant and user messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiMessage {
    /// Message role ("assistant" or "user")
    pub role: String,
    /// Message content blocks
    pub content: Vec<ContentBlock>,
}

/// Token usage information from the API
///
/// Tracks input and output token consumption.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageInfo {
    /// Number of input tokens consumed
    pub input_tokens: u32,
    /// Number of output tokens generated
    pub output_tokens: u32,
}

/// Information about an available tool
///
/// Provided in system init messages to describe callable tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    /// Tool name identifier
    pub name: String,
    /// Optional tool description
    #[serde(default)]
    pub description: Option<String>,
    /// Optional JSON schema for tool input
    #[serde(default)]
    pub input_schema: Option<serde_json::Value>,
}

/// Information about an MCP server
///
/// Provided in system init messages to describe connected MCP servers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerInfo {
    /// MCP server name identifier
    pub name: String,
    /// Additional server information fields
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_message_system_init() {
        let json = json!({
            "type": "system",
            "subtype": "init",
            "session_id": "sess_123",
            "tools": [{"name": "bash", "description": "Run shell commands"}],
            "mcp_servers": [{"name": "filesystem"}],
            "extra_field": "value"
        });

        let msg: Message = serde_json::from_value(json.clone()).unwrap();
        match &msg {
            Message::System(SystemMessage::Init {
                session_id,
                tools,
                mcp_servers,
                ..
            }) => {
                assert_eq!(session_id, "sess_123");
                assert_eq!(tools.len(), 1);
                assert_eq!(tools[0].name, "bash");
                assert_eq!(mcp_servers.len(), 1);
                assert_eq!(mcp_servers[0].name, "filesystem");
            }
            _ => panic!("Expected System::Init message"),
        }

        // Round-trip test
        let serialized = serde_json::to_value(&msg).unwrap();
        assert_eq!(serialized["type"], "system");
        assert_eq!(serialized["subtype"], "init");
    }

    #[test]
    fn test_message_system_compact_boundary() {
        let json = json!({
            "type": "system",
            "subtype": "compact_boundary"
        });

        let msg: Message = serde_json::from_value(json).unwrap();
        match msg {
            Message::System(SystemMessage::CompactBoundary) => {}
            _ => panic!("Expected System::CompactBoundary message"),
        }
    }

    #[test]
    fn test_message_assistant() {
        let json = json!({
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": [
                    {"type": "text", "text": "Hello!"}
                ]
            },
            "parent_tool_use_id": "tool_123",
            "duration_ms": 250
        });

        let msg: Message = serde_json::from_value(json).unwrap();
        match msg {
            Message::Assistant(assistant_msg) => {
                assert_eq!(assistant_msg.message.role, "assistant");
                assert_eq!(assistant_msg.message.content.len(), 1);
                assert_eq!(assistant_msg.parent_tool_use_id, Some("tool_123".to_string()));
                assert_eq!(assistant_msg.duration_ms, Some(250));
            }
            _ => panic!("Expected Assistant message"),
        }
    }

    #[test]
    fn test_message_user() {
        let json = json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": [
                    {"type": "text", "text": "Hello assistant!"}
                ]
            }
        });

        let msg: Message = serde_json::from_value(json).unwrap();
        match msg {
            Message::User(user_msg) => {
                assert_eq!(user_msg.message.role, "user");
                assert_eq!(user_msg.message.content.len(), 1);
            }
            _ => panic!("Expected User message"),
        }
    }

    #[test]
    fn test_message_result_success() {
        let json = json!({
            "type": "result",
            "subtype": "success",
            "result": "Task completed",
            "duration_ms": 1000,
            "num_turns": 5,
            "session_id": "sess_123",
            "total_cost_usd": 0.025,
            "usage": {
                "input_tokens": 100,
                "output_tokens": 50
            }
        });

        let msg: Message = serde_json::from_value(json).unwrap();
        match msg {
            Message::Result(ResultMessage::Success {
                result,
                duration_ms,
                num_turns,
                session_id,
                total_cost_usd,
                usage,
            }) => {
                assert_eq!(result, "Task completed");
                assert_eq!(duration_ms, Some(1000));
                assert_eq!(num_turns, Some(5));
                assert_eq!(session_id, Some("sess_123".to_string()));
                assert_eq!(total_cost_usd, Some(0.025));
                assert!(usage.is_some());
                let usage = usage.unwrap();
                assert_eq!(usage.input_tokens, 100);
                assert_eq!(usage.output_tokens, 50);
            }
            _ => panic!("Expected Result::Success message"),
        }
    }

    #[test]
    fn test_message_result_error() {
        let json = json!({
            "type": "result",
            "subtype": "error",
            "error": "Something went wrong",
            "code": 500
        });

        let msg: Message = serde_json::from_value(json).unwrap();
        match msg {
            Message::Result(ResultMessage::Error { error, extra }) => {
                assert_eq!(error, "Something went wrong");
                assert_eq!(extra["code"], 500);
            }
            _ => panic!("Expected Result::Error message"),
        }
    }

    #[test]
    fn test_message_result_input_required() {
        let json = json!({
            "type": "result",
            "subtype": "input_required"
        });

        let msg: Message = serde_json::from_value(json).unwrap();
        match msg {
            Message::Result(ResultMessage::InputRequired) => {}
            _ => panic!("Expected Result::InputRequired message"),
        }
    }

    #[test]
    fn test_content_block_text() {
        let json = json!({"type": "text", "text": "Hello world"});
        let block: ContentBlock = serde_json::from_value(json).unwrap();

        match block {
            ContentBlock::Text { text } => {
                assert_eq!(text, "Hello world");
            }
            _ => panic!("Expected Text block"),
        }
    }

    #[test]
    fn test_content_block_tool_use() {
        let json = json!({
            "type": "tool_use",
            "id": "tool_123",
            "name": "bash",
            "input": {"command": "ls -la"}
        });

        let block: ContentBlock = serde_json::from_value(json).unwrap();
        match block {
            ContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "tool_123");
                assert_eq!(name, "bash");
                assert_eq!(input["command"], "ls -la");
            }
            _ => panic!("Expected ToolUse block"),
        }
    }

    #[test]
    fn test_content_block_tool_result() {
        let json = json!({
            "type": "tool_result",
            "tool_use_id": "tool_123",
            "content": {"output": "file1.txt\nfile2.txt"},
            "is_error": false
        });

        let block: ContentBlock = serde_json::from_value(json).unwrap();
        match block {
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                assert_eq!(tool_use_id, "tool_123");
                assert_eq!(content["output"], "file1.txt\nfile2.txt");
                assert!(!is_error);
            }
            _ => panic!("Expected ToolResult block"),
        }
    }

    #[test]
    fn test_content_block_tool_result_default_is_error() {
        let json = json!({
            "type": "tool_result",
            "tool_use_id": "tool_123",
            "content": {"output": "success"}
        });

        let block: ContentBlock = serde_json::from_value(json).unwrap();
        match block {
            ContentBlock::ToolResult { is_error, .. } => {
                assert!(!is_error); // Should default to false
            }
            _ => panic!("Expected ToolResult block"),
        }
    }

    #[test]
    fn test_content_block_thinking() {
        let json = json!({
            "type": "thinking",
            "thinking": "Let me consider this..."
        });

        let block: ContentBlock = serde_json::from_value(json).unwrap();
        match block {
            ContentBlock::Thinking { thinking } => {
                assert_eq!(thinking, "Let me consider this...");
            }
            _ => panic!("Expected Thinking block"),
        }
    }

    #[test]
    fn test_stream_event() {
        let json = json!({
            "event_type": "message_start",
            "data": {"message_id": "msg_123"}
        });

        let event: StreamEvent = serde_json::from_value(json).unwrap();
        assert_eq!(event.event_type, "message_start");
        assert_eq!(event.data["message_id"], "msg_123");
    }

    #[test]
    fn test_usage_info() {
        let json = json!({
            "input_tokens": 100,
            "output_tokens": 50
        });

        let usage: UsageInfo = serde_json::from_value(json).unwrap();
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
    }

    #[test]
    fn test_tool_info_minimal() {
        let json = json!({"name": "bash"});
        let tool: ToolInfo = serde_json::from_value(json).unwrap();
        assert_eq!(tool.name, "bash");
        assert!(tool.description.is_none());
        assert!(tool.input_schema.is_none());
    }

    #[test]
    fn test_tool_info_full() {
        let json = json!({
            "name": "bash",
            "description": "Run shell commands",
            "input_schema": {
                "type": "object",
                "properties": {"command": {"type": "string"}}
            }
        });

        let tool: ToolInfo = serde_json::from_value(json).unwrap();
        assert_eq!(tool.name, "bash");
        assert_eq!(tool.description, Some("Run shell commands".to_string()));
        assert!(tool.input_schema.is_some());
    }

    #[test]
    fn test_mcp_server_info() {
        let json = json!({
            "name": "filesystem",
            "version": "1.0.0",
            "extra": "data"
        });

        let server: McpServerInfo = serde_json::from_value(json).unwrap();
        assert_eq!(server.name, "filesystem");
        assert_eq!(server.extra["version"], "1.0.0");
        assert_eq!(server.extra["extra"], "data");
    }

    #[test]
    fn test_optional_fields_default() {
        // Test that optional fields default correctly when missing
        let json = json!({
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": []
            }
        });

        let msg: Message = serde_json::from_value(json).unwrap();
        match msg {
            Message::Assistant(assistant_msg) => {
                assert!(assistant_msg.parent_tool_use_id.is_none());
                assert!(assistant_msg.duration_ms.is_none());
            }
            _ => panic!("Expected Assistant message"),
        }
    }

    #[test]
    fn test_json_round_trip_complex() {
        // Complex message with multiple content blocks
        let original = Message::Assistant(AssistantMessage {
            message: ApiMessage {
                role: "assistant".to_string(),
                content: vec![
                    ContentBlock::Text {
                        text: "I'll run that command.".to_string(),
                    },
                    ContentBlock::ToolUse {
                        id: "tool_xyz".to_string(),
                        name: "bash".to_string(),
                        input: json!({"command": "echo hello"}),
                    },
                    ContentBlock::Thinking {
                        thinking: "This should work...".to_string(),
                    },
                ],
            },
            parent_tool_use_id: None,
            duration_ms: Some(150),
        });

        // Serialize to JSON
        let json = serde_json::to_value(&original).unwrap();

        // Deserialize back
        let roundtrip: Message = serde_json::from_value(json).unwrap();

        // Verify structure is preserved
        match roundtrip {
            Message::Assistant(assistant_msg) => {
                assert_eq!(assistant_msg.message.content.len(), 3);
                assert_eq!(assistant_msg.duration_ms, Some(150));
            }
            _ => panic!("Expected Assistant message"),
        }
    }

    // === Fixture-Based Tests ===

    /// Helper function to load messages from a fixture file
    fn load_fixture(name: &str) -> Vec<Message> {
        use std::fs::File;
        use std::io::{BufRead, BufReader};

        let fixture_path = format!(
            "{}/tests/fixtures/{}.ndjson",
            env!("CARGO_MANIFEST_DIR"),
            name
        );
        let file = File::open(&fixture_path)
            .unwrap_or_else(|e| panic!("Failed to open fixture '{}': {}", fixture_path, e));

        BufReader::new(file)
            .lines()
            .enumerate()
            .map(|(i, line)| {
                let line = line.unwrap_or_else(|e| {
                    panic!("Failed to read line {} from fixture '{}': {}", i + 1, name, e)
                });
                serde_json::from_str(&line).unwrap_or_else(|e| {
                    panic!(
                        "Failed to parse line {} from fixture '{}': {}\nLine: {}",
                        i + 1,
                        name,
                        e,
                        line
                    )
                })
            })
            .collect()
    }

    #[test]
    fn test_simple_query_fixture() {
        let messages = load_fixture("simple_query");
        assert_eq!(messages.len(), 3, "Expected 3 messages in simple_query");

        // First message: System::Init
        match &messages[0] {
            Message::System(SystemMessage::Init {
                session_id,
                tools,
                mcp_servers,
                ..
            }) => {
                assert_eq!(session_id, "sess_simple_001");
                assert_eq!(tools.len(), 1);
                assert_eq!(tools[0].name, "bash");
                assert_eq!(mcp_servers.len(), 0);
            }
            _ => panic!("Expected System::Init as first message"),
        }

        // Second message: Assistant with Text content
        match &messages[1] {
            Message::Assistant(assistant_msg) => {
                assert_eq!(assistant_msg.message.role, "assistant");
                assert_eq!(assistant_msg.message.content.len(), 1);
                match &assistant_msg.message.content[0] {
                    ContentBlock::Text { text } => {
                        assert!(text.contains("README.md"));
                    }
                    _ => panic!("Expected Text content block"),
                }
                assert_eq!(assistant_msg.duration_ms, Some(142));
            }
            _ => panic!("Expected Assistant message as second message"),
        }

        // Third message: Result::Success
        match &messages[2] {
            Message::Result(ResultMessage::Success {
                result,
                duration_ms,
                num_turns,
                usage,
                ..
            }) => {
                assert_eq!(result, "Listed directory contents successfully");
                assert_eq!(*duration_ms, Some(156));
                assert_eq!(*num_turns, Some(1));
                assert!(usage.is_some());
                let usage = usage.as_ref().unwrap();
                assert_eq!(usage.input_tokens, 45);
                assert_eq!(usage.output_tokens, 28);
            }
            _ => panic!("Expected Result::Success as third message"),
        }
    }

    #[test]
    fn test_tool_use_fixture() {
        let messages = load_fixture("tool_use");
        assert_eq!(messages.len(), 5, "Expected 5 messages in tool_use");

        // First message: System::Init with tools
        match &messages[0] {
            Message::System(SystemMessage::Init {
                session_id,
                tools,
                mcp_servers,
                ..
            }) => {
                assert_eq!(session_id, "sess_tool_002");
                assert_eq!(tools.len(), 1);
                assert_eq!(tools[0].name, "bash");
                assert!(tools[0].input_schema.is_some());
                assert_eq!(mcp_servers.len(), 1);
                assert_eq!(mcp_servers[0].name, "filesystem");
            }
            _ => panic!("Expected System::Init"),
        }

        // Second message: Assistant with ToolUse
        match &messages[1] {
            Message::Assistant(assistant_msg) => {
                assert_eq!(assistant_msg.message.content.len(), 2);
                // First block: Text
                match &assistant_msg.message.content[0] {
                    ContentBlock::Text { text } => {
                        assert!(text.contains("check the current directory"));
                    }
                    _ => panic!("Expected Text block"),
                }
                // Second block: ToolUse
                match &assistant_msg.message.content[1] {
                    ContentBlock::ToolUse { id, name, input } => {
                        assert_eq!(id, "toolu_01ABC");
                        assert_eq!(name, "bash");
                        assert_eq!(input["command"], "ls -la");
                    }
                    _ => panic!("Expected ToolUse block"),
                }
            }
            _ => panic!("Expected Assistant message"),
        }

        // Third message: User with ToolResult
        match &messages[2] {
            Message::User(user_msg) => {
                assert_eq!(user_msg.message.content.len(), 1);
                match &user_msg.message.content[0] {
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => {
                        assert_eq!(tool_use_id, "toolu_01ABC");
                        assert!(!is_error);
                        let content_str = content.as_str().unwrap();
                        assert!(content_str.contains("Cargo.toml"));
                    }
                    _ => panic!("Expected ToolResult block"),
                }
            }
            _ => panic!("Expected User message"),
        }

        // Fourth message: Assistant response after tool
        match &messages[3] {
            Message::Assistant(assistant_msg) => {
                assert_eq!(assistant_msg.message.content.len(), 1);
                match &assistant_msg.message.content[0] {
                    ContentBlock::Text { text } => {
                        assert!(text.contains("Rust project"));
                    }
                    _ => panic!("Expected Text block"),
                }
            }
            _ => panic!("Expected Assistant message"),
        }

        // Fifth message: Result::Success
        match &messages[4] {
            Message::Result(ResultMessage::Success { num_turns, .. }) => {
                assert_eq!(*num_turns, Some(2));
            }
            _ => panic!("Expected Result::Success"),
        }
    }

    #[test]
    fn test_error_response_fixture() {
        let messages = load_fixture("error_response");
        assert_eq!(messages.len(), 3, "Expected 3 messages in error_response");

        // First message: System::Init
        match &messages[0] {
            Message::System(SystemMessage::Init { session_id, .. }) => {
                assert_eq!(session_id, "sess_error_003");
            }
            _ => panic!("Expected System::Init"),
        }

        // Second message: Assistant
        match &messages[1] {
            Message::Assistant(_) => {}
            _ => panic!("Expected Assistant message"),
        }

        // Third message: Result::Error with extra fields
        match &messages[2] {
            Message::Result(ResultMessage::Error { error, extra }) => {
                assert_eq!(error, "Failed to execute command: permission denied");
                assert_eq!(extra["error_code"], "EACCES");
                assert_eq!(extra["exit_code"], 126);
            }
            _ => panic!("Expected Result::Error"),
        }
    }

    #[test]
    fn test_thinking_content_fixture() {
        let messages = load_fixture("thinking_content");
        assert_eq!(messages.len(), 3, "Expected 3 messages in thinking_content");

        // First message: System::Init
        match &messages[0] {
            Message::System(SystemMessage::Init { session_id, .. }) => {
                assert_eq!(session_id, "sess_think_004");
            }
            _ => panic!("Expected System::Init"),
        }

        // Second message: Assistant with Thinking + Text
        match &messages[1] {
            Message::Assistant(assistant_msg) => {
                assert_eq!(assistant_msg.message.content.len(), 2);
                // First block: Thinking
                match &assistant_msg.message.content[0] {
                    ContentBlock::Thinking { thinking } => {
                        assert!(thinking.contains("analyze this request"));
                        assert!(thinking.contains("bash tool"));
                    }
                    _ => panic!("Expected Thinking block"),
                }
                // Second block: Text
                match &assistant_msg.message.content[1] {
                    ContentBlock::Text { text } => {
                        assert!(text.contains("list the files"));
                    }
                    _ => panic!("Expected Text block"),
                }
                assert_eq!(assistant_msg.duration_ms, Some(234));
            }
            _ => panic!("Expected Assistant message"),
        }

        // Third message: Result::Success
        match &messages[2] {
            Message::Result(ResultMessage::Success { usage, .. }) => {
                assert!(usage.is_some());
            }
            _ => panic!("Expected Result::Success"),
        }
    }

    #[test]
    fn test_all_fixtures_valid() {
        // Meta test: verify all fixtures parse without errors
        let fixtures = ["simple_query", "tool_use", "error_response", "thinking_content"];

        for fixture_name in &fixtures {
            let messages = load_fixture(fixture_name);
            assert!(
                !messages.is_empty(),
                "Fixture '{}' should contain at least one message",
                fixture_name
            );

            // Verify all messages have valid types
            for (i, msg) in messages.iter().enumerate() {
                match msg {
                    Message::System(_) | Message::Assistant(_) | Message::User(_)
                    | Message::Result(_) | Message::ControlRequest { .. }
                    | Message::ControlResponse { .. } => {}
                }
                // If we got here, the message is valid
                let _ = i; // Use i to avoid unused warning
            }
        }
    }

    // === Edge Case Tests ===

    #[test]
    fn test_empty_string_text_content() {
        let json = json!({"type": "text", "text": ""});
        let block: ContentBlock = serde_json::from_value(json).unwrap();

        match block {
            ContentBlock::Text { text } => {
                assert_eq!(text, "");
            }
            _ => panic!("Expected Text block"),
        }
    }

    #[test]
    fn test_empty_content_array() {
        let json = json!({
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": []
            }
        });

        let msg: Message = serde_json::from_value(json).unwrap();
        match msg {
            Message::Assistant(assistant_msg) => {
                assert_eq!(assistant_msg.message.content.len(), 0);
            }
            _ => panic!("Expected Assistant message"),
        }
    }

    #[test]
    fn test_minimal_system_init() {
        let json = json!({
            "type": "system",
            "subtype": "init",
            "session_id": "min_123",
            "tools": [],
            "mcp_servers": []
        });

        let msg: Message = serde_json::from_value(json).unwrap();
        match msg {
            Message::System(SystemMessage::Init {
                session_id,
                tools,
                mcp_servers,
                ..
            }) => {
                assert_eq!(session_id, "min_123");
                assert_eq!(tools.len(), 0);
                assert_eq!(mcp_servers.len(), 0);
            }
            _ => panic!("Expected System::Init"),
        }
    }

    #[test]
    fn test_large_tool_input() {
        // Create a complex nested JSON structure
        let complex_input = json!({
            "config": {
                "nested": {
                    "deeply": {
                        "structure": ["array", "of", "values"],
                        "number": 42,
                        "boolean": true
                    }
                },
                "large_array": vec!["item"; 100]
            },
            "metadata": {
                "timestamp": "2026-01-01T00:00:00Z",
                "version": "1.0.0"
            }
        });

        let json = json!({
            "type": "tool_use",
            "id": "tool_large",
            "name": "complex_tool",
            "input": complex_input
        });

        let block: ContentBlock = serde_json::from_value(json).unwrap();
        match block {
            ContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "tool_large");
                assert_eq!(name, "complex_tool");
                assert_eq!(input["config"]["nested"]["deeply"]["number"], 42);
                assert_eq!(
                    input["config"]["large_array"].as_array().unwrap().len(),
                    100
                );
            }
            _ => panic!("Expected ToolUse block"),
        }
    }

    #[test]
    fn test_unicode_in_text() {
        let json = json!({
            "type": "text",
            "text": "Hello 世界! 🚀 Emoji test: ✅ ❌ 🎉"
        });

        let block: ContentBlock = serde_json::from_value(json).unwrap();
        match block {
            ContentBlock::Text { text } => {
                assert!(text.contains("世界"));
                assert!(text.contains("🚀"));
                assert!(text.contains("✅"));
            }
            _ => panic!("Expected Text block"),
        }
    }
}

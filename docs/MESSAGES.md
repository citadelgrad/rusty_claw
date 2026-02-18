# Message Types Reference

This document describes the NDJSON message types exchanged between the rusty\_claw SDK and the Claude Code CLI. All messages are newline-delimited JSON (NDJSON), one JSON object per line.

**Source:** `crates/rusty_claw/src/messages.rs` and `crates/rusty_claw/src/control/messages.rs`

---

## Table of Contents

1. [Overview](#1-overview)
2. [Message Enum](#2-message-enum)
3. [SystemMessage](#3-systemmessage)
4. [AssistantMessage](#4-assistantmessage)
5. [ContentBlock](#5-contentblock)
6. [ResultMessage](#6-resultmessage)
7. [Supporting Types](#7-supporting-types)
8. [StreamEvent](#8-streamevent)
9. [Parsing Examples](#9-parsing-examples)
10. [Pattern Matching Examples](#10-pattern-matching-examples)
11. [JSON Wire Format Examples](#11-json-wire-format-examples)

---

## 1. Overview

A typical agent session follows this message flow:

```text
CLI -> SDK:  System::Init          (session starts, tools and MCP servers declared)
CLI -> SDK:  Assistant              (model responds with text, tool use, thinking)
CLI -> SDK:  User                   (tool results fed back as user messages)
CLI -> SDK:  Assistant              (model responds to tool results)
  ... (turns repeat) ...
CLI -> SDK:  Result::Success        (session ends with final result)
         or  Result::Error          (session ends with error)
         or  Result::InputRequired  (session pauses, waiting for user input)
```

Control messages (`ControlRequest` / `ControlResponse`) can be sent in either direction at any time during the session. `System::CompactBoundary` may appear when the CLI compacts conversation history.

All types derive `Debug`, `Clone`, `Serialize`, and `Deserialize`. Tagged enum variants use `#[serde(tag = "type", rename_all = "snake_case")]` or `#[serde(tag = "subtype", rename_all = "snake_case")]` for JSON discrimination.

---

## 2. Message Enum

The top-level `Message` enum is the entry point for all NDJSON lines. It is discriminated by the `"type"` field.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    System(SystemMessage),
    Assistant(AssistantMessage),
    User(UserMessage),
    Result(ResultMessage),
    ControlRequest {
        request_id: String,
        #[serde(flatten)]
        request: ControlRequest,
    },
    ControlResponse {
        request_id: String,
        #[serde(flatten)]
        response: ControlResponse,
    },
    RateLimitEvent(serde_json::Value),
    #[serde(rename = "mcp_message")]
    McpMessage(serde_json::Value),
}
```

| Variant | `"type"` value | Direction | Description |
|---------|---------------|-----------|-------------|
| `System` | `"system"` | CLI -> SDK | Lifecycle events (init, compact boundary) |
| `Assistant` | `"assistant"` | CLI -> SDK | Model response with content blocks |
| `User` | `"user"` | CLI -> SDK | User input or tool result messages |
| `Result` | `"result"` | CLI -> SDK | Final session outcome |
| `ControlRequest` | `"control_request"` | Bidirectional | Control protocol request |
| `ControlResponse` | `"control_response"` | Bidirectional | Control protocol response |
| `RateLimitEvent` | `"rate_limit_event"` | CLI -> SDK | Rate limit information from the CLI |
| `McpMessage` | `"mcp_message"` | CLI -> SDK | MCP message routed to SDK MCP server handler |

The `ControlRequest` and `ControlResponse` variants use `#[serde(flatten)]` on their payload, so the `"subtype"` field from the inner enum appears at the top level alongside `"type"` and `"request_id"`.

---

## 3. SystemMessage

Discriminated by the `"subtype"` field within a `"type": "system"` message.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum SystemMessage {
    Init {
        session_id: String,
        tools: Vec<ToolInfo>,
        mcp_servers: Vec<McpServerInfo>,
        #[serde(flatten)]
        extra: serde_json::Value,
    },
    CompactBoundary,
}
```

### Init

Sent once at session start. Declares the session ID, available tools, and connected MCP servers.

| Field | Type | Description |
|-------|------|-------------|
| `session_id` | `String` | Unique session identifier |
| `tools` | `Vec<ToolInfo>` | Available tool definitions |
| `mcp_servers` | `Vec<McpServerInfo>` | Connected MCP server information |
| `extra` | `Value` (flattened) | Additional fields from the CLI (forward-compatible) |

**Wire format:**

```json
{
  "type": "system",
  "subtype": "init",
  "session_id": "sess_abc123",
  "tools": [
    {
      "name": "bash",
      "description": "Run shell commands",
      "input_schema": {
        "type": "object",
        "properties": {
          "command": { "type": "string" }
        }
      }
    }
  ],
  "mcp_servers": [
    { "name": "filesystem" }
  ]
}
```

### CompactBoundary

Marker emitted when the CLI compacts conversation history. Contains no fields.

```json
{
  "type": "system",
  "subtype": "compact_boundary"
}
```

---

## 4. AssistantMessage

Wraps a model response from the Anthropic Messages API.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub message: ApiMessage,
    #[serde(default)]
    pub parent_tool_use_id: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `message` | `ApiMessage` | Yes | The API message with role and content blocks |
| `parent_tool_use_id` | `Option<String>` | No | Parent tool use ID if this is a nested agent response |
| `duration_ms` | `Option<u64>` | No | Duration of the API request in milliseconds |

The `message.role` field is always `"assistant"`. Content is delivered as a `Vec<ContentBlock>` (see [section 5](#5-contentblock)).

**Wire format:**

```json
{
  "type": "assistant",
  "message": {
    "role": "assistant",
    "content": [
      { "type": "text", "text": "I'll check that for you." },
      { "type": "tool_use", "id": "toolu_01ABC", "name": "bash", "input": { "command": "ls -la" } }
    ]
  },
  "parent_tool_use_id": null,
  "duration_ms": 250
}
```

### UserMessage

Wraps user input or tool results fed back into the conversation.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessage {
    pub message: ApiMessage,
}
```

The `message.role` field is always `"user"`. When carrying tool results, the content array contains `ContentBlock::ToolResult` items.

**Wire format (tool result):**

```json
{
  "type": "user",
  "message": {
    "role": "user",
    "content": [
      {
        "type": "tool_result",
        "tool_use_id": "toolu_01ABC",
        "content": "total 42\ndrwxr-xr-x  5 user staff  160 Jan  1 00:00 .\n-rw-r--r--  1 user staff 1234 Jan  1 00:00 Cargo.toml",
        "is_error": false
      }
    ]
  }
}
```

### ApiMessage

Shared structure used by both `AssistantMessage` and `UserMessage`.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiMessage {
    pub role: String,
    pub content: Vec<ContentBlock>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `role` | `String` | `"assistant"` or `"user"` |
| `content` | `Vec<ContentBlock>` | Ordered list of content blocks |

---

## 5. ContentBlock

Discriminated by the `"type"` field. Represents individual content items within an `ApiMessage`.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text { text: String },
    ToolUse { id: String, name: String, input: serde_json::Value },
    ToolResult { tool_use_id: String, content: serde_json::Value, #[serde(default)] is_error: bool },
    Thinking { thinking: String },
}
```

### Text

Plain text content from the model.

| Field | Type | Description |
|-------|------|-------------|
| `text` | `String` | The text content (may be empty) |

```json
{ "type": "text", "text": "Hello! I can help you with that." }
```

### ToolUse

A request from the model to invoke a tool.

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | Unique identifier for this tool invocation |
| `name` | `String` | Name of the tool to call |
| `input` | `Value` | Tool input parameters as a JSON object |

```json
{
  "type": "tool_use",
  "id": "toolu_01XYZ",
  "name": "bash",
  "input": { "command": "cat /etc/hostname" }
}
```

### ToolResult

The result of a tool invocation, sent back in a `User` message.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `tool_use_id` | `String` | -- | ID of the `ToolUse` this result corresponds to |
| `content` | `Value` | -- | Result data (string or structured JSON) |
| `is_error` | `bool` | `false` | Whether this result represents an error |

```json
{
  "type": "tool_result",
  "tool_use_id": "toolu_01XYZ",
  "content": "my-hostname",
  "is_error": false
}
```

Error example:

```json
{
  "type": "tool_result",
  "tool_use_id": "toolu_01XYZ",
  "content": { "output": "Permission denied" },
  "is_error": true
}
```

### Thinking

Extended thinking tokens from the model (when thinking is enabled).

| Field | Type | Description |
|-------|------|-------------|
| `thinking` | `String` | The model's internal reasoning text |

```json
{ "type": "thinking", "thinking": "Let me analyze this request. The user wants to list files, so I should use the bash tool." }
```

---

## 6. ResultMessage

Discriminated by the `"subtype"` field within a `"type": "result"` message. Represents the final outcome of an agent session.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum ResultMessage {
    Success {
        result: String,
        #[serde(default)] duration_ms: Option<u64>,
        #[serde(default)] num_turns: Option<u32>,
        #[serde(default)] session_id: Option<String>,
        #[serde(default)] total_cost_usd: Option<f64>,
        #[serde(default)] usage: Option<UsageInfo>,
    },
    Error {
        error: String,
        #[serde(flatten)] extra: serde_json::Value,
    },
    InputRequired,
}
```

### Success

Session completed normally.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `result` | `String` | Yes | Final result text |
| `duration_ms` | `Option<u64>` | No | Total execution duration in milliseconds |
| `num_turns` | `Option<u32>` | No | Number of conversation turns |
| `session_id` | `Option<String>` | No | Session identifier |
| `total_cost_usd` | `Option<f64>` | No | Total cost in USD |
| `usage` | `Option<UsageInfo>` | No | Aggregate token usage |

```json
{
  "type": "result",
  "subtype": "success",
  "result": "Task completed successfully",
  "duration_ms": 1500,
  "num_turns": 3,
  "session_id": "sess_abc123",
  "total_cost_usd": 0.025,
  "usage": { "input_tokens": 1200, "output_tokens": 350 }
}
```

### Error

Session ended due to an error.

| Field | Type | Description |
|-------|------|-------------|
| `error` | `String` | Human-readable error message |
| `extra` | `Value` (flattened) | Additional error context (error codes, details) |

```json
{
  "type": "result",
  "subtype": "error",
  "error": "Failed to execute command: permission denied",
  "error_code": "EACCES",
  "exit_code": 126
}
```

The `extra` field is flattened, so additional keys like `"error_code"` and `"exit_code"` appear at the top level.

### InputRequired

Session is paused and waiting for additional user input. Contains no fields beyond the discriminator.

```json
{
  "type": "result",
  "subtype": "input_required"
}
```

---

## 7. Supporting Types

### UsageInfo

Token consumption counters.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageInfo {
    pub input_tokens: u32,
    pub output_tokens: u32,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `input_tokens` | `u32` | Number of input tokens consumed |
| `output_tokens` | `u32` | Number of output tokens generated |

```json
{ "input_tokens": 1200, "output_tokens": 350 }
```

### ToolInfo

Tool definition provided in `SystemMessage::Init`.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub input_schema: Option<serde_json::Value>,
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | `String` | Yes | Tool name identifier |
| `description` | `Option<String>` | No | Human-readable tool description |
| `input_schema` | `Option<Value>` | No | JSON Schema for tool input parameters |

Minimal:

```json
{ "name": "bash" }
```

Full:

```json
{
  "name": "bash",
  "description": "Run shell commands",
  "input_schema": {
    "type": "object",
    "properties": {
      "command": { "type": "string" }
    }
  }
}
```

### McpServerInfo

MCP server metadata provided in `SystemMessage::Init`.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerInfo {
    pub name: String,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | MCP server name identifier |
| `extra` | `Value` (flattened) | Additional server information fields |

```json
{ "name": "filesystem", "version": "1.0.0" }
```

The `extra` field is flattened, so keys like `"version"` appear at the top level alongside `"name"`.

---

## 8. StreamEvent

Used for real-time streaming updates during agent execution.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    pub event_type: String,
    pub data: serde_json::Value,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `event_type` | `String` | Event type identifier (e.g., `"message_start"`) |
| `data` | `Value` | Event-specific payload |

```json
{ "event_type": "message_start", "data": { "message_id": "msg_abc123" } }
```

---

## 9. Parsing Examples

### Deserialize a single NDJSON line

```rust
use rusty_claw::messages::Message;

let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Hello!"}]}}"#;
let msg: Message = serde_json::from_str(line).unwrap();
```

### Parse a stream of NDJSON lines

```rust
use std::io::{BufRead, BufReader};
use rusty_claw::messages::Message;

fn parse_ndjson(reader: impl std::io::Read) -> Vec<Message> {
    BufReader::new(reader)
        .lines()
        .map(|line| {
            let line = line.expect("failed to read line");
            serde_json::from_str(&line).expect("failed to parse message")
        })
        .collect()
}
```

### Load test fixtures

The crate ships NDJSON test fixtures in `tests/fixtures/`:

- `simple_query.ndjson` -- Basic query/response exchange with system init
- `tool_use.ndjson` -- Complete tool invocation cycle with ToolUse and ToolResult
- `error_response.ndjson` -- Error result handling scenario
- `thinking_content.ndjson` -- Extended thinking tokens with ContentBlock::Thinking

```rust
use std::fs::File;
use std::io::{BufRead, BufReader};
use rusty_claw::messages::Message;

let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/simple_query.ndjson");
let file = File::open(path).unwrap();
let messages: Vec<Message> = BufReader::new(file)
    .lines()
    .map(|line| serde_json::from_str(&line.unwrap()).unwrap())
    .collect();
```

---

## 10. Pattern Matching Examples

### Handle all top-level message variants

```rust
use rusty_claw::messages::{Message, SystemMessage, ResultMessage, ContentBlock};

fn handle_message(msg: Message) {
    match msg {
        Message::System(system_msg) => match system_msg {
            SystemMessage::Init { session_id, tools, mcp_servers, .. } => {
                println!("Session {session_id} started with {} tools", tools.len());
                for tool in &tools {
                    println!("  Tool: {}", tool.name);
                }
                for server in &mcp_servers {
                    println!("  MCP server: {}", server.name);
                }
            }
            SystemMessage::CompactBoundary => {
                println!("Conversation history compacted");
            }
        },

        Message::Assistant(assistant_msg) => {
            if let Some(parent_id) = &assistant_msg.parent_tool_use_id {
                println!("Nested response for tool {parent_id}");
            }
            for block in &assistant_msg.message.content {
                match block {
                    ContentBlock::Text { text } => println!("Text: {text}"),
                    ContentBlock::ToolUse { id, name, input } => {
                        println!("Tool call {id}: {name}({input})");
                    }
                    ContentBlock::ToolResult { tool_use_id, content, is_error } => {
                        let status = if *is_error { "ERROR" } else { "OK" };
                        println!("Tool result [{status}] for {tool_use_id}: {content}");
                    }
                    ContentBlock::Thinking { thinking } => {
                        println!("Thinking: {thinking}");
                    }
                }
            }
        }

        Message::User(user_msg) => {
            println!("User message with {} content blocks", user_msg.message.content.len());
        }

        Message::Result(result_msg) => match result_msg {
            ResultMessage::Success { result, duration_ms, num_turns, usage, .. } => {
                println!("Success: {result}");
                if let Some(ms) = duration_ms {
                    println!("  Duration: {ms}ms");
                }
                if let Some(turns) = num_turns {
                    println!("  Turns: {turns}");
                }
                if let Some(usage) = usage {
                    println!("  Tokens: {} in / {} out", usage.input_tokens, usage.output_tokens);
                }
            }
            ResultMessage::Error { error, extra } => {
                println!("Error: {error}");
                if let Some(code) = extra.get("error_code") {
                    println!("  Code: {code}");
                }
            }
            ResultMessage::InputRequired => {
                println!("Waiting for user input...");
            }
        },

        Message::ControlRequest { request_id, request } => {
            println!("Control request {request_id}: {request:?}");
        }

        Message::ControlResponse { request_id, response } => {
            println!("Control response {request_id}: {response:?}");
        }

        Message::RateLimitEvent(data) => {
            println!("Rate limit event: {data}");
        }

        Message::McpMessage(data) => {
            println!("MCP message: {data}");
        }
    }
}
```

### Extract tool use calls from an assistant message

```rust
use rusty_claw::messages::{Message, ContentBlock};

fn extract_tool_calls(msg: &Message) -> Vec<(&str, &str, &serde_json::Value)> {
    match msg {
        Message::Assistant(assistant_msg) => {
            assistant_msg.message.content.iter()
                .filter_map(|block| match block {
                    ContentBlock::ToolUse { id, name, input } => {
                        Some((id.as_str(), name.as_str(), input))
                    }
                    _ => None,
                })
                .collect()
        }
        _ => vec![],
    }
}
```

---

## 11. JSON Wire Format Examples

### Complete session: simple query

```json
{"type":"system","subtype":"init","session_id":"sess_001","tools":[{"name":"bash","description":"Run shell commands"}],"mcp_servers":[]}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Here are the files in the current directory: README.md, Cargo.toml, src/"}]},"duration_ms":142}
{"type":"result","subtype":"success","result":"Listed directory contents successfully","duration_ms":156,"num_turns":1,"usage":{"input_tokens":45,"output_tokens":28}}
```

### Complete session: tool use cycle

```json
{"type":"system","subtype":"init","session_id":"sess_002","tools":[{"name":"bash","description":"Run shell commands","input_schema":{"type":"object","properties":{"command":{"type":"string"}}}}],"mcp_servers":[{"name":"filesystem"}]}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Let me check the current directory."},{"type":"tool_use","id":"toolu_01ABC","name":"bash","input":{"command":"ls -la"}}]},"duration_ms":180}
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_01ABC","content":"total 42\n-rw-r--r--  1 user staff 1234 Cargo.toml\ndrwxr-xr-x  3 user staff   96 src","is_error":false}]}}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"This is a Rust project with a Cargo.toml and src directory."}]},"duration_ms":120}
{"type":"result","subtype":"success","result":"Analyzed project structure","duration_ms":450,"num_turns":2,"usage":{"input_tokens":200,"output_tokens":85}}
```

### Error result

```json
{"type":"result","subtype":"error","error":"Failed to execute command: permission denied","error_code":"EACCES","exit_code":126}
```

### Thinking content

```json
{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"Let me analyze this request. The user wants to list files, so I should use the bash tool."},{"type":"text","text":"I'll list the files for you."}]},"duration_ms":234}
```

### Control request (SDK -> CLI)

```json
{"type":"control_request","request_id":"req_001","subtype":"initialize","can_use_tool":true}
```

### Control response (CLI -> SDK)

```json
{"type":"control_response","request_id":"req_001","subtype":"success"}
```

### Control request (CLI -> SDK: can\_use\_tool)

```json
{"type":"control_request","request_id":"req_002","subtype":"can_use_tool","tool_name":"Bash","tool_input":{"command":"rm -rf /"}}
```

### Control response (SDK -> CLI: deny tool)

```json
{"type":"control_response","request_id":"req_002","subtype":"success","allowed":false,"reason":"Dangerous command blocked"}
```

### Input required

```json
{"type":"result","subtype":"input_required"}
```

### Compact boundary

```json
{"type":"system","subtype":"compact_boundary"}
```

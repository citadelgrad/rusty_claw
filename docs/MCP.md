# MCP Integration

Model Context Protocol (MCP) integration in rusty_claw. This document covers both external MCP servers (managed by the Claude CLI) and SDK MCP servers (in-process Rust tools).

## 1. Overview

rusty_claw supports two categories of MCP servers:

- **External MCP servers** -- Separate processes (stdio, SSE, HTTP) whose lifecycle is managed by the Claude CLI. Configured via `mcp_servers: HashMap<String, McpServerConfig>` in `ClaudeAgentOptions`.
- **SDK MCP servers** -- In-process tools written in Rust that run inside the SDK. The CLI sends JSON-RPC requests over the control protocol and the SDK routes them to the appropriate tool handler.

Tool names visible to Claude follow the convention `mcp__{server_name}__{tool_name}` (double underscores).

**Source files:**
- `crates/rusty_claw/src/mcp_server.rs` -- `SdkMcpServerImpl`, `SdkMcpTool`, `ToolHandler`, `ToolResult`, `ToolContent`, `SdkMcpServerRegistry`
- `crates/rusty_claw/src/options.rs` -- `McpServerConfig`, `SdkMcpServer`, builder methods
- `crates/rusty_claw_macros/src/lib.rs` -- `#[claw_tool]` proc macro

---

## 2. External MCP Servers

External MCP servers are configured in `ClaudeAgentOptions` and their lifecycle is managed entirely by the Claude CLI. The SDK passes the configuration through; it does not spawn or communicate with external servers directly.

### Current implementation

`McpServerConfig` is currently a placeholder struct in `options.rs`:

```rust
/// MCP server configuration (placeholder for future MCP tasks)
#[derive(Debug, Clone)]
pub struct McpServerConfig {
    // Detailed implementation in future tasks (SPEC.md section 7.1)
}
```

### Spec target (SPEC.md section 7.1)

The full implementation will be a tagged enum supporting three transport types:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum McpServerConfig {
    Stdio {
        command: String,
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
    },
    Sse {
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
    },
    Http {
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
    },
}
```

### Builder usage

```rust
use std::collections::HashMap;
use rusty_claw::options::{ClaudeAgentOptions, McpServerConfig};

let mut servers = HashMap::new();
servers.insert("my-server".to_string(), McpServerConfig { /* ... */ });

let options = ClaudeAgentOptions::builder()
    .mcp_servers(servers)
    .build();
```

---

## 3. SDK MCP Servers (In-Process Tools)

SDK MCP servers host Rust functions as tools that Claude can invoke. They run inside the SDK process. The Claude CLI sends JSON-RPC messages via the control protocol's `mcp_message` handler, and the SDK routes them to the correct server and tool.

### Architecture

```
CLI  --[mcp_message]--> ControlProtocol
                            |
                    SdkMcpServerRegistry  (implements McpMessageHandler)
                            |
                    SdkMcpServerImpl      (routes JSON-RPC methods)
                            |
                    SdkMcpTool            (wraps ToolHandler)
                            |
                    ToolHandler::call()   (user-defined async fn)
```

### Core types

```rust
pub struct SdkMcpServerImpl {
    pub name: String,
    pub version: String,
    tools: HashMap<String, SdkMcpTool>,
}

pub struct SdkMcpTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    handler: Arc<dyn ToolHandler>,
}

#[async_trait]
pub trait ToolHandler: Send + Sync {
    async fn call(&self, args: Value) -> Result<ToolResult, ClawError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: Vec<ToolContent>,
    #[serde(rename = "isError", skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ToolContent {
    Text { text: String },
    Image { data: String, #[serde(rename = "mimeType")] mime_type: String },
}
```

### Convenience constructors

```rust
// Text result (success)
let result = ToolResult::text("Operation completed");

// Error result (is_error = true)
let result = ToolResult::error("Something went wrong");

// Multi-content result
let result = ToolResult::new(vec![
    ToolContent::text("Description of image"),
    ToolContent::image("base64data...", "image/png"),
]);
```

### SdkMcpServerImpl methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `(name, version) -> Self` | Create a new server |
| `register_tool` | `(&mut self, tool: SdkMcpTool)` | Register a tool (overwrites on duplicate name) |
| `get_tool` | `(&self, name: &str) -> Option<&SdkMcpTool>` | Look up a tool by name |
| `list_tools` | `(&self) -> Vec<Value>` | Return tool definitions for `tools/list` |
| `handle_jsonrpc` | `(&self, request: Value) -> Result<Value, ClawError>` | Route a JSON-RPC request |

### SdkMcpServerRegistry

Manages multiple `SdkMcpServerImpl` instances keyed by name. Implements `McpMessageHandler` so it can be registered with the control protocol.

```rust
let mut registry = SdkMcpServerRegistry::new();
let server = SdkMcpServerImpl::new("my_server", "1.0.0");
registry.register(server);

// Called by the control protocol when a mcp_message arrives:
let response = registry.handle("my_server", json_rpc_request).await?;
```

---

## 4. Creating Tools with `#[claw_tool]`

The `#[claw_tool]` proc macro (in `crates/rusty_claw_macros`) transforms an async function into a tool definition. It generates:

1. A handler struct implementing `ToolHandler`
2. A builder function (same name as the original function) that returns `SdkMcpTool`
3. A JSON Schema for the input parameters (derived from the function signature)

### Macro attributes

| Attribute | Required | Default |
|-----------|----------|---------|
| `name` | No | Function name with `_` replaced by `-` |
| `description` | No | Doc comment, or `"Tool: {name}"` |

### Supported parameter types

| Rust Type | JSON Schema Type | Notes |
|-----------|-----------------|-------|
| `String` | `string` | |
| `i8`..`i128`, `u8`..`u128`, `f32`, `f64` | `number` | |
| `bool` | `boolean` | |
| `Option<T>` | schema of `T` | Not included in `required` array |
| `Vec<T>` | `array` with `items` of `T` | |
| Custom types | `object` | Fallback |

### Requirements

- Function must be `async`
- Return type must be `ToolResult` or `Result<ToolResult, E>`
- No `self` parameter

### Example

```rust
use rusty_claw::prelude::*;
use rusty_claw::claw_tool;

#[claw_tool(
    name = "calculator",
    description = "Perform basic arithmetic operations (add or multiply)"
)]
async fn calculator(operation: String, a: i32, b: i32) -> ToolResult {
    match operation.as_str() {
        "add" => ToolResult::text(format!("Result: {}", a + b)),
        "multiply" => ToolResult::text(format!("Result: {}", a * b)),
        _ => ToolResult::error(format!("Unknown operation: {}", operation)),
    }
}

// The macro generates a function that returns SdkMcpTool:
let tool = calculator();
assert_eq!(tool.name, "calculator");
```

### With optional parameters

```rust
#[claw_tool(
    name = "echo",
    description = "Echo text back, optionally repeated multiple times"
)]
async fn echo(text: String, repeat: Option<i32>) -> ToolResult {
    let times = repeat.unwrap_or(1);
    let result = (0..times)
        .map(|_| text.clone())
        .collect::<Vec<_>>()
        .join("\n");
    ToolResult::text(result)
}
```

### With `Result` return type

```rust
#[claw_tool(name = "fetch", description = "Fetch a URL")]
async fn fetch_url(url: String) -> Result<ToolResult, ClawError> {
    if url.is_empty() {
        return Err(ClawError::ToolExecution("URL cannot be empty".into()));
    }
    Ok(ToolResult::text(format!("Fetched: {}", url)))
}
```

---

## 5. Manual Tool Creation with `ToolHandler`

For cases where the proc macro is insufficient, implement `ToolHandler` directly.

```rust
use rusty_claw::prelude::*;
use rusty_claw::mcp_server::{SdkMcpTool, ToolHandler, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

struct CalculatorHandler;

#[async_trait]
impl ToolHandler for CalculatorHandler {
    async fn call(&self, args: Value) -> Result<ToolResult, ClawError> {
        let a = args["a"].as_f64().unwrap_or(0.0);
        let b = args["b"].as_f64().unwrap_or(0.0);
        Ok(ToolResult::text(format!("Result: {}", a + b)))
    }
}

let tool = SdkMcpTool::new(
    "add",
    "Add two numbers",
    json!({
        "type": "object",
        "properties": {
            "a": { "type": "number" },
            "b": { "type": "number" }
        },
        "required": ["a", "b"]
    }),
    Arc::new(CalculatorHandler),
);

let mut server = SdkMcpServerImpl::new("calculator", "1.0.0");
server.register_tool(tool);
```

Manual creation is useful when the tool handler needs to capture state (closures or struct fields with shared data via `Arc`).

---

## 6. Tool Naming Convention

Claude sees MCP tools with qualified names in the format:

```
mcp__{server_name}__{tool_name}
```

For example, a tool named `calculator` on a server named `math-tools` appears as:

```
mcp__math-tools__calculator
```

This naming is handled by the CLI, not the SDK. Inside the SDK, tools are registered with their short name only.

---

## 7. Allowing MCP Tools

By default, the CLI may require permission to use tools. Use `allowed_tools` in `ClaudeAgentOptions` to pre-authorize specific MCP tools:

```rust
let options = ClaudeAgentOptions::builder()
    .allowed_tools(vec![
        "mcp__math-tools__calculator".to_string(),
        "mcp__math-tools__add".to_string(),
    ])
    .build();
```

This passes `--allowed-tools=mcp__math-tools__calculator,mcp__math-tools__add` to the CLI.

You can also use `disallowed_tools` to block specific tools:

```rust
let options = ClaudeAgentOptions::builder()
    .disallowed_tools(vec!["mcp__risky-server__delete".to_string()])
    .build();
```

---

## 8. JSON-RPC Routing

`SdkMcpServerImpl::handle_jsonrpc()` routes incoming JSON-RPC 2.0 requests by method name:

| Method | Handler | Description |
|--------|---------|-------------|
| `initialize` | `handle_initialize` | Returns protocol version (`2025-11-25`), capabilities (`{ "tools": {} }`), and server info (name, version) |
| `tools/list` | `handle_tools_list` | Returns array of tool definitions (name, description, inputSchema) |
| `tools/call` | `handle_tools_call` | Looks up tool by `params.name`, executes with `params.arguments`, returns result |
| (anything else) | -- | Returns JSON-RPC error code `-32601` (Method not found) |

### Request/response examples

**initialize:**

```json
// Request
{"jsonrpc": "2.0", "id": 1, "method": "initialize"}

// Response
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocolVersion": "2025-11-25",
    "capabilities": { "tools": {} },
    "serverInfo": { "name": "my_server", "version": "1.0.0" }
  }
}
```

**tools/list:**

```json
// Request
{"jsonrpc": "2.0", "id": 2, "method": "tools/list"}

// Response
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "tools": [
      {
        "name": "calculator",
        "description": "Add two numbers",
        "inputSchema": {
          "type": "object",
          "properties": { "a": {"type": "number"}, "b": {"type": "number"} },
          "required": ["a", "b"]
        }
      }
    ]
  }
}
```

**tools/call:**

```json
// Request
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": { "name": "calculator", "arguments": { "a": 2, "b": 3 } }
}

// Response
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "content": [{"type": "text", "text": "Result: 5"}]
  }
}
```

---

## 9. Error Handling

Errors surface at multiple layers:

### JSON-RPC error codes

| Code | Meaning | When |
|------|---------|------|
| `-32601` | Method not found | Unknown JSON-RPC method |
| `-32602` | Invalid params | Tool not found by name |
| `-32603` | Internal error | Tool handler returned `Err(ClawError)` |

### ToolResult errors

A tool can return a successful JSON-RPC response that still indicates a logical error via `is_error`:

```rust
// This is a successful JSON-RPC response, but the tool reports an error
Ok(ToolResult::error("Invalid input: expected positive number"))
```

### ClawError propagation

If `ToolHandler::call()` returns `Err(ClawError)`, the server wraps it in a JSON-RPC error response with code `-32603`. The `#[claw_tool]` macro generates parameter extraction code that returns `ClawError::ToolExecution` for missing or invalid required parameters.

---

## 10. Full End-to-End Example

```rust
use rusty_claw::prelude::*;
use rusty_claw::claw_tool;
use rusty_claw::mcp_server::{SdkMcpServerImpl, SdkMcpServerRegistry};
use rusty_claw::options::SdkMcpServer;
use std::sync::Arc;

// Define tools with the proc macro
#[claw_tool(name = "calculator", description = "Perform arithmetic")]
async fn calculator(operation: String, a: i32, b: i32) -> ToolResult {
    match operation.as_str() {
        "add" => ToolResult::text(format!("{}", a + b)),
        "multiply" => ToolResult::text(format!("{}", a * b)),
        _ => ToolResult::error(format!("Unknown operation: {}", operation)),
    }
}

// Build the server and registry
let mut server = SdkMcpServerImpl::new("math-tools", "1.0.0");
server.register_tool(calculator());

let mut registry = SdkMcpServerRegistry::new();
registry.register(server);

// Configure the agent options
let options = ClaudeAgentOptions::builder()
    .sdk_mcp_servers(vec![SdkMcpServer {
        name: "math-tools".to_string(),
        version: "1.0.0".to_string(),
    }])
    .allowed_tools(vec!["mcp__math-tools__calculator".to_string()])
    .build();

// Register with client (after creating ClaudeClient):
// client.register_mcp_message_handler(Arc::new(registry)).await;
```

---

## 11. Related Docs

- [SPEC.md](SPEC.md) -- Section 7: MCP Integration (full protocol spec)
- [QUICKSTART.md](QUICKSTART.md) -- Getting started guide
- [HOOKS.md](HOOKS.md) -- Lifecycle hooks (can match on `mcp__*` tool names)
- `examples/custom_tool.rs` -- Working example with two tools

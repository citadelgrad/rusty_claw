# Technical Specification: rusty_claw

> Detailed architecture and implementation spec for the Rust Claude Agent SDK

**Version:** 0.1.0-draft
**Date:** 2026-02-12
**Status:** Draft
**Reference:** Anthropic's [claude-agent-sdk-python](https://github.com/anthropics/claude-agent-sdk-python) (MIT License)

---

## 1. System Architecture

### 1.1 High-Level Overview

```
┌─────────────────────────────────────────────┐
│              User Application               │
│                                             │
│  ┌─────────┐  ┌──────────────┐  ┌────────┐ │
│  │ query() │  │ ClaudeClient │  │ Hooks  │ │
│  └────┬────┘  └──────┬───────┘  └───┬────┘ │
│       │              │              │       │
│  ┌────▼──────────────▼──────────────▼────┐  │
│  │           Control Protocol            │  │
│  │  (JSON-RPC request/response routing)  │  │
│  └───────────────┬───────────────────────┘  │
│                  │                          │
│  ┌───────────────▼───────────────────────┐  │
│  │         Transport Layer               │  │
│  │   (trait: connect/read/write/close)   │  │
│  └───────────────┬───────────────────────┘  │
│                  │                          │
│  ┌───────────────▼───────────────────────┐  │
│  │    SubprocessCLITransport (default)   │  │
│  │  spawns claude CLI as child process   │  │
│  │  stdin ◄──► stdout (NDJSON)           │  │
│  └───────────────────────────────────────┘  │
│                                             │
│  ┌───────────────────────────────────────┐  │
│  │        SDK MCP Server Bridge          │  │
│  │  (in-process tool hosting via MCP)    │  │
│  └───────────────────────────────────────┘  │
└─────────────────────────────────────────────┘
              │
              │ subprocess stdin/stdout
              ▼
    ┌──────────────────┐
    │  Claude Code CLI │
    │   (>= 2.0.0)    │
    └──────────────────┘
```

### 1.2 Crate Structure

```
rusty_claw/                         # Workspace root
├── Cargo.toml                      # Workspace definition
├── crates/
│   ├── rusty_claw/                 # Main library crate
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs              # Public API re-exports & prelude
│   │   │   ├── query.rs            # query() one-shot function
│   │   │   ├── client.rs           # ClaudeClient for multi-turn sessions
│   │   │   ├── transport/
│   │   │   │   ├── mod.rs          # Transport trait definition
│   │   │   │   ├── subprocess.rs   # SubprocessCLITransport implementation
│   │   │   │   └── discovery.rs    # CLI path finding & version validation
│   │   │   ├── control/
│   │   │   │   ├── mod.rs          # ControlProtocol handler & routing
│   │   │   │   ├── messages.rs     # Control request/response types
│   │   │   │   ├── handlers.rs     # Handler registry (can_use_tool, hooks, MCP)
│   │   │   │   └── pending.rs      # Pending request tracking with ID mapping
│   │   │   ├── hooks/
│   │   │   │   ├── mod.rs          # Hook registry and dispatcher
│   │   │   │   ├── types.rs        # HookInput, HookContext
│   │   │   │   ├── callback.rs     # HookCallback trait + blanket impl
│   │   │   │   └── response.rs     # HookResponse, PermissionDecision
│   │   │   ├── mcp_server.rs       # In-process MCP server & JSON-RPC routing
│   │   │   ├── permissions/
│   │   │   │   ├── mod.rs
│   │   │   │   └── handler.rs      # DefaultPermissionHandler
│   │   │   ├── messages.rs         # Message, ContentBlock, SystemMessage, etc.
│   │   │   ├── options.rs          # ClaudeAgentOptions builder
│   │   │   ├── error.rs            # ClawError hierarchy (thiserror)
│   │   │   └── types.rs            # Shared type definitions
│   │   ├── examples/
│   │   │   ├── simple_query.rs
│   │   │   ├── interactive_client.rs
│   │   │   ├── custom_tool.rs
│   │   │   ├── hooks_guardrails.rs
│   │   │   └── subagent_usage.rs
│   │   └── tests/
│   │       ├── integration_test.rs
│   │       ├── mock_cli.rs         # Mock CLI subprocess for testing
│   │       └── fixtures/           # NDJSON message fixtures for replay
│   └── rusty_claw_macros/          # Proc macro crate
│       ├── Cargo.toml
│       └── src/
│           └── lib.rs              # #[claw_tool] attribute macro
├── docs/                           # Documentation
│   ├── QUICKSTART.md               # Step-by-step tutorial
│   ├── HOOKS.md                    # Hook system guide
│   ├── SESSIONS.md                 # Session management guide
│   ├── MCP.md                      # MCP integration guide
│   ├── PERMISSIONS.md              # Permission modes and rules
│   ├── SUBAGENTS.md                # Subagent definition and usage
│   ├── MESSAGES.md                 # Message types reference
│   ├── SPEC.md                     # Technical specification
│   └── PRD.md                      # Product requirements
└── CONTRIBUTING.md                 # Development guide
```

---

## 2. Transport Layer

### 2.1 Transport Trait

```rust
use async_trait::async_trait;
use tokio::sync::mpsc;

/// Abstract transport for communicating with Claude Code CLI.
/// The default implementation spawns a subprocess, but this trait
/// enables custom transports (remote connections, mock for testing).
#[async_trait]
pub trait Transport: Send + Sync {
    /// Establish the connection (spawn process, open socket, etc.)
    async fn connect(&mut self) -> Result<(), ClawError>;

    /// Write a JSON message to the CLI's stdin
    async fn write(&self, message: &[u8]) -> Result<(), ClawError>;

    /// Returns a receiver for incoming NDJSON messages from stdout.
    /// Messages are parsed by the transport into raw `serde_json::Value`.
    fn messages(&self) -> mpsc::UnboundedReceiver<Result<serde_json::Value, ClawError>>;

    /// Signal end of input (close stdin)
    async fn end_input(&self) -> Result<(), ClawError>;

    /// Close the transport and clean up resources
    async fn close(&mut self) -> Result<(), ClawError>;

    /// Whether the transport is connected and ready
    fn is_ready(&self) -> bool;
}
```

### 2.2 SubprocessCLITransport

Spawns `claude` CLI as a child process with the following arguments:

```
claude \
  --output-format=stream-json \    # NDJSON output
  --verbose \                       # Include system messages
  --max-turns={N} \                 # From options
  --model={model} \                 # From options
  --permission-mode={mode} \        # From options
  --input-format=stream-json \      # Enable control protocol
  --settings-sources="" \           # Isolation: no external settings
  -p "{prompt}"                     # Initial prompt
```

**Implementation details:**
- Uses `tokio::process::Command` with piped stdin/stdout/stderr
- Spawns a background `tokio::task` to read stdout line-by-line and parse JSON
- Stderr is captured for error reporting but not parsed as messages
- On `Drop`, sends SIGTERM then waits up to 5s before SIGKILL

### 2.3 Message Framing

Messages are **newline-delimited JSON** (NDJSON). Each line is a complete JSON object:

```
{"type":"system","subtype":"init","session_id":"abc123",...}\n
{"type":"assistant","message":{"role":"assistant","content":[...]},...}\n
{"type":"result","subtype":"success","result":"...",...}\n
```

The transport reads lines, parses each as `serde_json::Value`, and sends through the channel.

---

## 3. Message Types

### 3.1 Message Enum

```rust
/// Top-level message from the Claude Code CLI.
/// Discriminated on the `type` field in JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    System(SystemMessage),
    Assistant(AssistantMessage),
    User(UserMessage),
    Result(ResultMessage),
}

/// System-level events (init, session info, compaction boundaries)
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

/// An assistant turn containing content blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub message: ApiMessage,
    #[serde(default)]
    pub parent_tool_use_id: Option<String>,
    /// Duration of the API call in ms
    #[serde(default)]
    pub duration_ms: Option<u64>,
}

/// Content blocks within a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: serde_json::Value,
        #[serde(default)]
        is_error: bool,
    },
    Thinking {
        thinking: String,
    },
}

/// The final result of a query.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum ResultMessage {
    Success {
        result: String,
        #[serde(default)]
        duration_ms: Option<u64>,
        #[serde(default)]
        num_turns: Option<u32>,
        #[serde(default)]
        session_id: Option<String>,
        #[serde(default)]
        total_cost_usd: Option<f64>,
        #[serde(default)]
        usage: Option<UsageInfo>,
    },
    Error {
        error: String,
        #[serde(flatten)]
        extra: serde_json::Value,
    },
    InputRequired,
}
```

### 3.2 Stream Events (Partial Messages)

When `include_partial_messages` is enabled, the CLI emits `StreamEvent` messages containing raw Anthropic API streaming events:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    pub event_type: String,
    pub data: serde_json::Value,
}
```

These are passed through without interpretation — consumers use them for real-time UI updates.

---

## 4. Control Protocol

### 4.1 Overview

The control protocol enables bidirectional communication between the SDK and CLI. It's layered on top of the same stdin/stdout NDJSON channel as messages. Control messages have `type: "control_request"` or `type: "control_response"`.

### 4.2 Protocol Handler

```rust
pub struct ControlProtocol {
    transport: Arc<dyn Transport>,
    /// Pending requests awaiting responses, keyed by request_id
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<ControlResponse>>>>,
    /// Registered handlers for incoming requests from CLI
    handlers: Arc<Mutex<ControlHandlers>>,
}

struct ControlHandlers {
    can_use_tool: Option<Box<dyn CanUseToolHandler>>,
    hook_callbacks: HashMap<String, Box<dyn HookHandler>>,
    mcp_message: Option<Box<dyn McpMessageHandler>>,
}

impl ControlProtocol {
    /// Send a control request and wait for the response.
    pub async fn request(&self, request: ControlRequest) -> Result<ControlResponse, ClawError> {
        let id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id.clone(), tx);

        let msg = json!({
            "type": "control_request",
            "request_id": id,
            "request": request,
        });
        self.transport.write(serde_json::to_vec(&msg)?.as_slice()).await?;

        let response = tokio::time::timeout(Duration::from_secs(30), rx).await??;
        Ok(response)
    }

    /// Handle an incoming control request from the CLI.
    pub async fn handle_incoming(&self, request_id: &str, request: IncomingControlRequest) {
        // Route to appropriate handler, send response back
    }
}
```

### 4.3 Control Request Types

#### SDK → CLI (Outgoing)

| Subtype | Purpose | Fields |
|---------|---------|--------|
| `initialize` | Handshake after connection | `hooks`, `agents`, `sdk_mcp_servers`, `permissions`, `can_use_tool` |
| `interrupt` | Cancel current operation | (none) |
| `set_permission_mode` | Change permission mode | `mode: String` |
| `set_model` | Switch model | `model: String` |
| `mcp_status` | Query MCP server status | (none) |
| `rewind_files` | Restore file checkpoints | `message_id: String` |

#### CLI → SDK (Incoming)

| Subtype | Purpose | Fields |
|---------|---------|--------|
| `can_use_tool` | Permission check callback | `tool_name`, `tool_input` |
| `hook_callback` | Execute registered hook | `hook_id`, `hook_event`, `hook_input` |
| `mcp_message` | Route MCP request to SDK server | `server_name`, `message` (JSON-RPC) |

### 4.4 Initialization Sequence

```
1. SDK spawns CLI with --input-format=stream-json
2. SDK sends: control_request { subtype: "initialize", hooks: {...}, agents: {...}, ... }
3. CLI responds: control_response { subtype: "success", ... }
4. CLI sends: system message { subtype: "init", session_id: "...", tools: [...] }
5. SDK sends prompt via stdin (or control_request for client mode)
6. CLI begins processing and streaming messages back
```

---

## 5. Options & Configuration

### 5.1 ClaudeAgentOptions

```rust
/// Configuration for a Claude agent session.
/// Uses the builder pattern — all fields have sensible defaults.
#[derive(Debug, Clone, Default)]
pub struct ClaudeAgentOptions {
    // -- Prompt & behavior --
    pub system_prompt: Option<SystemPrompt>,
    pub append_system_prompt: Option<String>,
    pub max_turns: Option<u32>,
    pub model: Option<String>,

    // -- Tools & permissions --
    pub allowed_tools: Vec<String>,
    pub disallowed_tools: Vec<String>,
    pub permission_mode: Option<PermissionMode>,
    pub permission_prompt_tool_allowlist: Vec<String>,

    // -- MCP --
    pub mcp_servers: HashMap<String, McpServerConfig>,
    pub sdk_mcp_servers: Vec<SdkMcpServer>,

    // -- Hooks --
    pub hooks: HashMap<HookEvent, Vec<HookMatcher>>,

    // -- Subagents --
    pub agents: HashMap<String, AgentDefinition>,

    // -- Session --
    pub resume: Option<String>,
    pub fork_session: bool,
    pub session_name: Option<String>,
    pub enable_file_checkpointing: bool,

    // -- Environment --
    pub cwd: Option<PathBuf>,
    pub cli_path: Option<PathBuf>,
    pub env: HashMap<String, String>,

    // -- Settings isolation --
    pub settings_sources: Option<Vec<String>>,

    // -- Output --
    pub output_format: Option<serde_json::Value>,
    pub include_partial_messages: bool,

    // -- Advanced --
    pub betas: Vec<String>,
    pub sandbox_settings: Option<SandboxSettings>,
}

#[derive(Debug, Clone)]
pub enum SystemPrompt {
    Custom(String),
    Preset { preset: String },
}

#[derive(Debug, Clone)]
pub enum PermissionMode {
    Default,
    AcceptEdits,
    BypassPermissions,
    Plan,
}

#[derive(Debug, Clone)]
pub struct AgentDefinition {
    pub description: String,
    pub prompt: String,
    pub tools: Vec<String>,
    pub model: Option<String>,
}
```

Builder pattern via `derive_builder` or hand-rolled:

```rust
let options = ClaudeAgentOptions::builder()
    .allowed_tools(vec!["Read", "Bash"])
    .permission_mode(PermissionMode::AcceptEdits)
    .max_turns(5)
    .build();
```

---

## 6. Hook System

### 6.1 Hook Events

```rust
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    UserPromptSubmit,
    Stop,
    SubagentStop,
    SubagentStart,
    PreCompact,
    Notification,
    PermissionRequest,
}
```

### 6.2 Hook Registration

```rust
#[derive(Debug, Clone)]
pub struct HookMatcher {
    /// Tool name pattern to match (e.g., "Bash", "mcp__*")
    pub tool_name: Option<String>,
    /// The callback function
    pub callback: Arc<dyn HookCallback>,
}

#[async_trait]
pub trait HookCallback: Send + Sync {
    async fn call(
        &self,
        input: HookInput,
        tool_use_id: Option<&str>,
        context: &HookContext,
    ) -> Result<HookResponse, ClawError>;
}
```

### 6.3 Hook Response

```rust
#[derive(Debug, Clone, Default, Serialize)]
pub struct HookResponse {
    /// Decision for PreToolUse hooks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_decision: Option<PermissionDecision>,

    /// Reason for the decision
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_decision_reason: Option<String>,

    /// Additional context to inject
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_context: Option<String>,

    /// Whether to continue execution
    #[serde(rename = "continue", default = "default_true")]
    pub should_continue: bool,

    /// Modified tool input (for input transformation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_input: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionDecision {
    Allow,
    Deny,
    Ask,
}
```

---

## 7. MCP Integration

### 7.1 External MCP Servers

Configured via `mcp_servers` in options. The CLI manages their lifecycle.

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

### 7.2 SDK MCP Servers (In-Process)

The SDK hosts MCP tools inside the Rust process. The CLI sends JSON-RPC requests via the control protocol, and the SDK routes them to the appropriate tool handler.

```rust
pub struct SdkMcpServer {
    pub name: String,
    pub version: String,
    tools: Vec<SdkMcpTool>,
}

pub struct SdkMcpTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub handler: Box<dyn ToolHandler>,
}

#[async_trait]
pub trait ToolHandler: Send + Sync {
    async fn call(&self, args: serde_json::Value) -> Result<ToolResult, ClawError>;
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolResult {
    pub content: Vec<ToolContent>,
    #[serde(default)]
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ToolContent {
    Text { text: String },
    Image { data: String, mime_type: String },
}
```

### 7.3 MCP JSON-RPC Routing

The SDK manually routes MCP methods (no full MCP SDK dependency needed):

| Method | Action |
|--------|--------|
| `initialize` | Return server info and capabilities |
| `tools/list` | Return registered tool definitions |
| `tools/call` | Invoke the named tool's handler |

```rust
impl SdkMcpServer {
    pub async fn handle_jsonrpc(&self, request: serde_json::Value) -> serde_json::Value {
        let method = request["method"].as_str().unwrap_or("");
        match method {
            "initialize" => self.handle_initialize(&request),
            "tools/list" => self.handle_tools_list(&request),
            "tools/call" => self.handle_tools_call(&request).await,
            _ => json_rpc_error(-32601, "Method not found"),
        }
    }
}
```

### 7.4 Proc Macro for Tool Definitions

The `rusty_claw_macros` crate provides `#[claw_tool]`:

```rust
// Input:
#[claw_tool(name = "lookup_user", description = "Look up a user by ID")]
async fn lookup_user(user_id: String) -> ToolResult {
    ToolResult::text(format!("Found user: {user_id}"))
}

// Expands to:
fn lookup_user() -> SdkMcpTool {
    SdkMcpTool {
        name: "lookup_user".to_string(),
        description: "Look up a user by ID".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "user_id": { "type": "string" }
            },
            "required": ["user_id"]
        }),
        handler: Box::new(LookupUserHandler),
    }
}

struct LookupUserHandler;

#[async_trait]
impl ToolHandler for LookupUserHandler {
    async fn call(&self, args: serde_json::Value) -> Result<ToolResult, ClawError> {
        let user_id: String = serde_json::from_value(args["user_id"].clone())?;
        // Original function body
        Ok(ToolResult::text(format!("Found user: {user_id}")))
    }
}
```

---

## 8. Error Handling

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClawError {
    #[error("Claude Code CLI not found. Install it or set cli_path.")]
    CliNotFound,

    #[error("Failed to connect to Claude Code CLI: {0}")]
    Connection(String),

    #[error("CLI process exited with code {code}: {stderr}")]
    Process {
        code: i32,
        stderr: String,
    },

    #[error("Failed to parse JSON from CLI: {0}")]
    JsonDecode(#[from] serde_json::Error),

    #[error("Failed to parse message: {reason}")]
    MessageParse {
        reason: String,
        raw: String,
    },

    #[error("Control protocol timeout waiting for {subtype}")]
    ControlTimeout {
        subtype: String,
    },

    #[error("Control protocol error: {0}")]
    ControlError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Tool execution failed: {0}")]
    ToolExecution(String),
}
```

---

## 9. CLI Discovery & Version Check

```rust
pub struct CliDiscovery;

impl CliDiscovery {
    /// Find the Claude Code CLI binary.
    /// Search order:
    /// 1. Explicit `cli_path` from options
    /// 2. `CLAUDE_CLI_PATH` environment variable
    /// 3. `claude` in PATH (via `which`)
    /// 4. Common install locations:
    ///    - ~/.claude/local/claude
    ///    - /usr/local/bin/claude
    ///    - ~/.npm/bin/claude
    pub async fn find(cli_path: Option<&Path>) -> Result<PathBuf, ClawError> { ... }

    /// Validate CLI version >= 2.0.0
    /// Runs: claude --version
    /// Parses semver from output
    pub async fn validate_version(cli: &Path) -> Result<String, ClawError> { ... }
}
```

---

## 10. Testing Strategy

### 10.1 Unit Tests

- **Message parsing**: Deserialize known JSON payloads into `Message` variants. Cover all content block types, edge cases (empty content, unknown fields via `#[serde(flatten)]`).
- **Control protocol**: Verify request/response serialization, ID matching, timeout handling.
- **Options builder**: Verify all fields serialize correctly to CLI arguments.
- **Hook matching**: Verify tool name pattern matching, hook ordering.

### 10.2 Integration Tests

- **Mock CLI process**: A small binary (`tests/mock_cli.rs`) that accepts the same arguments as the real CLI and replays canned NDJSON responses. Tests run against this mock.
- **End-to-end with real CLI**: Gated behind `#[cfg(feature = "integration")]` and CI environment variable. Requires Claude Code CLI installed.

### 10.3 Test Fixtures

Store representative NDJSON message sequences in `tests/fixtures/`:

```
tests/fixtures/
├── simple_query.ndjson       # Basic question/answer
├── tool_use.ndjson           # Tool use + result cycle
├── multi_turn.ndjson         # Multi-turn conversation
├── error_response.ndjson     # Error scenarios
├── hook_callback.ndjson      # Hook invocation flow
└── mcp_tool_call.ndjson      # MCP tool invocation
```

---

## 11. Dependencies

| Crate | Purpose | Version Constraint |
|-------|---------|-------------------|
| `tokio` | Async runtime | ^1.35 (with `full` features) |
| `serde` | Serialization framework | ^1 |
| `serde_json` | JSON parsing | ^1 |
| `thiserror` | Error derive macros | ^2 |
| `uuid` | Request ID generation | ^1 (with `v4` feature) |
| `tokio-stream` | Stream utilities | ^0.1 |
| `tracing` | Structured logging | ^0.1 |
| `async-trait` | Async trait support | ^0.1 |

**Proc macro crate additional deps:**

| Crate | Purpose |
|-------|---------|
| `syn` | Rust syntax parsing |
| `quote` | Code generation |
| `proc-macro2` | Proc macro utilities |

### 11.1 Dependency Philosophy

- **Minimal surface**: Only add dependencies that provide significant value
- **No MCP SDK dependency**: Route MCP JSON-RPC manually (3 methods, simple protocol)
- **No HTTP client**: The SDK never makes HTTP calls — it talks to the CLI via subprocess
- **Feature-gated extras**: Optional features for `async-std`, extended logging, etc.

---

## 12. Release Plan

| Phase | Version | Scope | Milestone |
|-------|---------|-------|-----------|
| **Foundation** | 0.1.0 | Transport, messages, `query()`, errors, CLI discovery | Can run simple queries |
| **Interactive** | 0.2.0 | `ClaudeClient`, control protocol, hooks, permissions, sessions | Can build interactive agents |
| **Full SDK** | 0.3.0 | SDK MCP servers, proc macros, subagents, structured output | Feature parity with Python SDK |
| **Polish** | 0.4.0 | Documentation, examples, benchmarks, crates.io publish | Production-ready |

---

## 13. Implementation Notes

### 13.1 Rust-Specific Considerations

1. **Async trait methods**: Use `async-trait` crate until Rust stabilizes async fn in traits for dyn dispatch scenarios. For static dispatch, use `impl Future` directly.

2. **Callback ergonomics**: Hooks and tool handlers use trait objects (`Box<dyn HookCallback>`) for runtime flexibility. Provide `impl<F, Fut> HookCallback for F where F: Fn(...) -> Fut` blanket impl so closures work directly.

3. **Serde tagged enums**: The `#[serde(tag = "type")]` attribute maps perfectly to the CLI's discriminated JSON messages. Use `#[serde(flatten)]` for extensibility (unknown fields captured as `serde_json::Value`).

4. **Stream vs Iterator**: All message sequences are `tokio_stream::Stream`. The `query()` function returns `impl Stream<Item = Result<Message, ClawError>>`. Use `StreamExt` for ergonomic consumption.

5. **Process lifecycle**: `SubprocessCLITransport` must handle:
   - Graceful shutdown (close stdin, wait for exit)
   - Forced shutdown (SIGTERM → SIGKILL after timeout)
   - Unexpected exit detection (background task monitors process)
   - Stderr capture for diagnostics

6. **Reserved keywords**: Unlike Python (`async_`, `continue_`), Rust has no conflicts with the control protocol field names. Use raw identifiers (`r#type`) only for `type` field if needed, but `#[serde(rename)]` is cleaner.

### 13.2 Performance Considerations

- **Zero-copy parsing**: Use `serde_json::from_str` with borrowed data where possible. For long-running sessions, avoid accumulating old messages.
- **Channel backpressure**: The transport → protocol channel is unbounded (messages are small). If memory is a concern, switch to bounded channel with configurable buffer size.
- **Proc macro compilation**: The `rusty_claw_macros` crate compiles independently. Keep it minimal to avoid slowing down builds.

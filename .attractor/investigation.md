# Investigation: rusty_claw-tlh - Implement SDK MCP Server bridge

**Task ID:** rusty_claw-tlh
**Status:** IN_PROGRESS
**Date:** 2026-02-13

---

## Executive Summary

This task implements the **MCP (Model Context Protocol) server bridge** within the Rusty Claw SDK. The MCP server bridge enables SDK users to register Rust functions as tools that can be invoked by Claude via the MCP protocol. The implementation includes:

1. **SdkMcpServer** - Main MCP server struct managing tool registry and JSON-RPC routing
2. **SdkMcpTool** - Tool wrapper containing metadata and handler reference
3. **ToolHandler** trait - Async trait for tool execution
4. **ToolResult/ToolContent** types - Result representation for MCP responses
5. **JSON-RPC routing** - Handler methods for `initialize`, `tools/list`, `tools/call`

---

## Current State Analysis

### What Exists (Solid Foundation)

#### 1. Control Protocol Infrastructure ✅
**File:** `crates/rusty_claw/src/control/mod.rs`

The Control Protocol implementation provides:
- **Request/response routing** - `ControlProtocol::request()` for sending requests to CLI
- **Handler dispatch** - `ControlProtocol::handle_incoming()` routes CLI requests to registered handlers
- **McpMessageHandler trait** (lines 186-199) - Already defined for routing MCP messages:
  ```rust
  #[async_trait]
  pub trait McpMessageHandler: Send + Sync {
      async fn handle(&self, server_name: &str, message: Value) -> Result<Value, ClawError>;
  }
  ```
- **Handler registration** - `ControlHandlers::register_mcp_message()` (line 341)
- **Control message routing** - `IncomingControlRequest::McpMessage` variant (lines 305-311)

**Key Insight:** The infrastructure for **routing MCP messages from CLI → SDK** already exists! We just need to implement the handler that:
1. Deserializes JSON-RPC requests
2. Routes to appropriate SdkMcpServer method
3. Serializes JSON-RPC responses

#### 2. SdkMcpServer Placeholder ✅
**File:** `crates/rusty_claw/src/options.rs` (lines 95-98)

Current placeholder:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkMcpServer {
    // Detailed implementation in future tasks (SPEC.md section 7.2)
}
```

**Action Required:** Expand this with full implementation

#### 3. Control Messages Support ✅
**File:** `crates/rusty_claw/src/control/messages.rs`

The `Initialize` control request already supports `sdk_mcp_servers`:
```rust
Initialize {
    hooks: HashMap<HookEvent, Vec<HookMatcher>>,
    agents: HashMap<String, AgentDefinition>,
    sdk_mcp_servers: Vec<SdkMcpServer>,  // ✅ Already included
    permissions: Option<PermissionMode>,
    can_use_tool: bool,
}
```

**Result:** When we create an `SdkMcpServer`, it will automatically be sent to the CLI during initialization!

#### 4. Dependencies ✅
All required dependencies are already in `Cargo.toml`:
- ✅ `tokio` - Async runtime
- ✅ `async-trait` - Async trait support
- ✅ `serde` + `serde_json` - Serialization
- ✅ `uuid` - Request ID generation (if needed)

**No new dependencies required!**

---

## Required Implementation

### Architecture Overview

```text
┌────────────────────────────────────────────────────────────────┐
│                     MCP Server Bridge                          │
│                                                                │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │                 SdkMcpServer                             │ │
│  │                                                          │ │
│  │  pub name: String                                        │ │
│  │  pub version: String                                     │ │
│  │  tools: Vec<SdkMcpTool>                                  │ │
│  │                                                          │ │
│  │  Methods:                                                │ │
│  │  - new() → Self                                          │ │
│  │  - register_tool(tool: SdkMcpTool)                       │ │
│  │  - handle_jsonrpc(request: Value) → Value                │ │
│  │  - handle_initialize(request: &Value) → Value            │ │
│  │  - handle_tools_list(request: &Value) → Value            │ │
│  │  - handle_tools_call(request: &Value) → Result<Value>    │ │
│  └──────────────────────────────────────────────────────────┘ │
│                          │                                     │
│                          │ Contains Vec<SdkMcpTool>            │
│                          ▼                                     │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │                 SdkMcpTool                               │ │
│  │                                                          │ │
│  │  pub name: String                                        │ │
│  │  pub description: String                                 │ │
│  │  pub input_schema: serde_json::Value                     │ │
│  │  handler: Arc<dyn ToolHandler>                           │ │
│  │                                                          │ │
│  │  Methods:                                                │ │
│  │  - new(name, description, schema, handler) → Self        │ │
│  │  - to_tool_definition() → serde_json::Value              │ │
│  │  - execute(args: Value) → Result<ToolResult>             │ │
│  └──────────────────────────────────────────────────────────┘ │
│                          │                                     │
│                          │ Uses Arc<dyn ToolHandler>           │
│                          ▼                                     │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │         ToolHandler (async trait)                        │ │
│  │                                                          │ │
│  │  async fn call(&self, args: Value)                       │ │
│  │      → Result<ToolResult, ClawError>                     │ │
│  └──────────────────────────────────────────────────────────┘ │
│                          │                                     │
│                          │ Returns                             │
│                          ▼                                     │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │              ToolResult + ToolContent                    │ │
│  │                                                          │ │
│  │  pub struct ToolResult {                                 │ │
│  │      pub content: Vec<ToolContent>,                      │ │
│  │      pub is_error: bool,                                 │ │
│  │  }                                                       │ │
│  │                                                          │ │
│  │  pub enum ToolContent {                                  │ │
│  │      Text { text: String },                              │ │
│  │      Image { data: String, mime_type: String },          │ │
│  │  }                                                       │ │
│  └──────────────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────────────┘
                         │
                         │ Registered via McpMessageHandler
                         ▼
        ┌────────────────────────────────────────┐
        │       ControlProtocol                  │
        │   (existing infrastructure)            │
        │                                        │
        │  - handle_incoming() routes            │
        │    IncomingControlRequest::McpMessage  │
        │  - Calls McpMessageHandler::handle()   │
        └────────────────────────────────────────┘
                         │
                         │ JSON-RPC messages from CLI
                         ▼
              ┌──────────────────────┐
              │    Claude Code CLI   │
              │   (MCP client)       │
              └──────────────────────┘
```

### Message Flow

**1. Registration (Initialization):**
```
SDK creates SdkMcpServer
  → Register with ClaudeAgentOptions
  → ControlProtocol::initialize() sends to CLI
  → CLI knows about SDK-hosted MCP server
```

**2. Tool List Request:**
```
CLI sends JSON-RPC: {"method": "tools/list", ...}
  → IncomingControlRequest::McpMessage
  → ControlProtocol::handle_incoming()
  → McpMessageHandler::handle()
  → SdkMcpServer::handle_jsonrpc()
  → SdkMcpServer::handle_tools_list()
  → Returns tool definitions JSON
```

**3. Tool Execution:**
```
CLI sends JSON-RPC: {"method": "tools/call", "params": {...}}
  → IncomingControlRequest::McpMessage
  → ControlProtocol::handle_incoming()
  → McpMessageHandler::handle()
  → SdkMcpServer::handle_jsonrpc()
  → SdkMcpServer::handle_tools_call()
  → Find tool by name
  → SdkMcpTool::execute()
  → ToolHandler::call()
  → Returns ToolResult
  → Serialize to JSON-RPC response
```

---

## Implementation Plan

### Phase 1: Core Types (90 min)

**File:** `crates/rusty_claw/src/mcp_server.rs` (NEW)

#### 1.1 ToolContent enum (~15 min)
- Text variant with text field
- Image variant with data + mime_type fields
- Serde tagging for JSON-RPC format
- Helper constructors

#### 1.2 ToolResult struct (~15 min)
- content: Vec<ToolContent> field
- is_error: bool field
- Helper constructors: text(), error()

#### 1.3 ToolHandler trait (~30 min)
- Async trait for tool execution
- Thread-safe (Send + Sync)
- Takes JSON args, returns ToolResult

#### 1.4 Tests (~30 min)
- Test ToolContent serialization
- Test ToolResult creation
- Test ToolHandler trait with mock

---

### Phase 2: SdkMcpTool (60 min)

#### 2.1 SdkMcpTool struct (~20 min)
- name, description, input_schema fields
- Arc-wrapped handler for shared ownership

#### 2.2 Methods (~20 min)
- new() constructor
- to_tool_definition() for JSON-RPC
- execute() delegates to handler

#### 2.3 Tests (~20 min)
- Test tool creation
- Test to_tool_definition() serialization
- Test execute() with mock handler

---

### Phase 3: SdkMcpServer Core (90 min)

#### 3.1 Update SdkMcpServer in options.rs (~15 min)
- Add name and version fields
- Keep minimal for serialization

#### 3.2 Full SdkMcpServerImpl (~45 min)
- Tool registry (HashMap)
- register_tool(), get_tool(), list_tools()

#### 3.3 Tests (~30 min)
- Test server creation
- Test tool registration
- Test list_tools() output

---

### Phase 4: JSON-RPC Routing (120 min)

#### 4.1 JSON-RPC helpers (~20 min)
- json_rpc_success()
- json_rpc_error()

#### 4.2-4.5 Handler methods (~80 min)
- handle_initialize()
- handle_tools_list()
- handle_tools_call()
- handle_jsonrpc() router

#### 4.6 Tests (~20 min)
- Test each handler method
- Test JSON-RPC routing

---

### Phase 5: McpMessageHandler Integration (60 min)

#### 5.1 Server registry (~30 min)
- SdkMcpServerRegistry struct
- register(), get() methods

#### 5.2 Implement McpMessageHandler (~30 min)
- Route messages to servers
- Handle server not found errors

---

### Phase 6: Module Integration (30 min)

#### 6.1 Update lib.rs (~10 min)
- Replace mcp {} stub with real module
- Add prelude exports

#### 6.2 Update options.rs (~20 min)
- Finalize SdkMcpServer definition

---

### Phase 7: Comprehensive Tests (120 min)

#### 7.1 Unit tests (~60 min)
- 12 unit tests for all components

#### 7.2 Integration tests (~60 min)
- 6 integration tests for full flows
- Thread safety validation

**Target:** 20-30 comprehensive tests

---

### Phase 8: Documentation (60 min)

#### 8.1 Module-level docs (~20 min)
- Overview with architecture diagram
- Quick start example

#### 8.2 API documentation (~30 min)
- Document all public types
- Add doctests for key methods

#### 8.3 README example (~10 min)
- End-to-end usage example

---

### Phase 9: Verification (30 min)

#### 9.1 Test execution (~10 min)
- cargo test --lib
- cargo test --doc

#### 9.2 Clippy (~10 min)
- cargo clippy -- -D warnings

#### 9.3 Documentation check (~10 min)
- cargo doc --no-deps --open

---

## Files to Create/Modify

### New Files (1 file, ~800-1000 lines)

**1. `crates/rusty_claw/src/mcp_server.rs`**
- ToolContent enum (~30 lines)
- ToolResult struct (~40 lines)
- ToolHandler trait (~20 lines)
- SdkMcpTool struct + impl (~100 lines)
- SdkMcpServerImpl struct + impl (~200 lines)
- SdkMcpServerRegistry struct + impl (~100 lines)
- JSON-RPC helpers (~50 lines)
- Module-level documentation (~80 lines)
- Tests (~300-400 lines)

### Modified Files (2 files, ~15 lines total)

**2. `crates/rusty_claw/src/lib.rs`** (+5 lines)
- Replace `pub mod mcp {}` stub with `pub mod mcp_server;`
- Add mcp_server exports to prelude

**3. `crates/rusty_claw/src/options.rs`** (~10 lines)
- Update `SdkMcpServer` struct from placeholder to full definition
- Add `name` and `version` fields

---

## Success Criteria

### 1. ✅ SdkMcpServer struct with MCP protocol support
- SdkMcpServerImpl with tool registry
- JSON-RPC message handling
- Protocol version support

### 2. ✅ Tool registry and listing functionality
- register_tool() method
- list_tools() method
- get_tool() lookup

### 3. ✅ Tool execution via ToolHandler trait
- ToolHandler trait definition
- SdkMcpTool::execute() implementation
- Error propagation

### 4. ✅ JSON-RPC routing for all MCP methods
- initialize → server info + capabilities
- tools/list → tool definitions
- tools/call → tool execution
- Method routing via handle_jsonrpc()

### 5. ✅ Proper error handling and responses
- JSON-RPC error codes (-32601, -32602, -32603)
- ClawError propagation
- ToolResult error flag

### 6. ✅ Integration with Control Protocol handler
- SdkMcpServerRegistry implements McpMessageHandler
- Routes messages to appropriate server
- Returns JSON-RPC responses

### 7. ✅ 20-30 comprehensive tests
- 12 unit tests
- 6 integration tests
- Doctests for key methods
- **Total: ~20-25 tests**

### 8. ✅ Complete documentation with examples
- Module-level docs with architecture
- API documentation for all public types
- Doctests for key methods
- End-to-end usage example

### 9. ✅ Zero clippy warnings
- All code passes clippy -- -D warnings

---

## Dependencies & Risks

### Dependencies

#### External Dependencies ✅
**All already in Cargo.toml:**
- tokio - Async runtime
- async-trait - Async trait support
- serde + serde_json - Serialization
- uuid - (Optional) Request tracking

**No new dependencies needed!**

#### Internal Dependencies ✅
- ✅ **rusty_claw-91n** (Control Protocol handler) - **COMPLETED**
- ✅ Transport layer - Already implemented
- ✅ Error types - Already implemented
- ✅ Options types - Already implemented

### Risks & Mitigation

#### 1. JSON-RPC Protocol Compliance
**Risk:** Incorrect JSON-RPC 2.0 format could break CLI communication

**Mitigation:**
- Use explicit JSON-RPC helper functions
- Validate against MCP specification
- Comprehensive tests for all response formats

#### 2. Tool Handler Thread Safety
**Risk:** ToolHandler trait must be Send + Sync for concurrent execution

**Mitigation:**
- Enforce Send + Sync bounds in trait definition
- Use Arc<dyn ToolHandler> for shared ownership
- Test concurrent execution

#### 3. Error Propagation Layers
**Risk:** Errors can occur at multiple layers

**Mitigation:**
- Clear error propagation path: ClawError → JSON-RPC error codes
- Test error cases at each layer
- Document error handling strategy

---

## Implementation Timeline

**Total Estimated Time:** ~9.5 hours

| Phase | Duration | Dependencies |
|-------|----------|--------------|
| 1. Core Types | 90 min | None |
| 2. SdkMcpTool | 60 min | Phase 1 |
| 3. SdkMcpServer Core | 90 min | Phase 2 |
| 4. JSON-RPC Routing | 120 min | Phase 3 |
| 5. Integration | 60 min | Phase 4 |
| 6. Module Integration | 30 min | Phase 5 |
| 7. Tests | 120 min | Phase 6 |
| 8. Documentation | 60 min | Phase 7 |
| 9. Verification | 30 min | Phase 8 |

---

## Downstream Impact

**This task unblocks 2 P2/P3 tasks:**

### 1. rusty_claw-zyo - Implement #[claw_tool] proc macro [P2]
**Why Blocked:** Proc macro needs SdkMcpTool and ToolHandler trait to generate code

### 2. rusty_claw-bkm - Write examples [P3]
**Why Blocked:** Examples need working MCP server to demonstrate tool registration

---

## Example Usage

```rust
use rusty_claw::prelude::*;
use async_trait::async_trait;
use serde_json::{json, Value};

// Define a tool handler
struct CalculatorHandler;

#[async_trait]
impl ToolHandler for CalculatorHandler {
    async fn call(&self, args: Value) -> Result<ToolResult, ClawError> {
        let a = args["a"].as_f64().unwrap_or(0.0);
        let b = args["b"].as_f64().unwrap_or(0.0);
        let result = a + b;
        Ok(ToolResult::text(format!("Result: {}", result)))
    }
}

// Create a tool
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

// Create and register server
let mut server = SdkMcpServerImpl::new("calculator", "1.0.0");
server.register_tool(tool);

// Register with registry
let mut registry = SdkMcpServerRegistry::new();
registry.register(server);

// Register as McpMessageHandler
control_protocol
    .handlers()
    .await
    .register_mcp_message(Arc::new(registry));
```

---

## Investigation Complete ✅

**Status:** Ready to begin **Phase 1: Core Types**

**Next Steps:**
1. Create `crates/rusty_claw/src/mcp_server.rs`
2. Implement ToolContent enum
3. Implement ToolResult struct
4. Implement ToolHandler trait
5. Write initial tests

**Estimated Completion:** Phase 1 complete in ~90 minutes

---

*Investigation conducted: 2026-02-13*
*Ready for implementation: ✅*

# Investigation: rusty_claw-91n - Implement Control Protocol handler

## Task Summary

**ID:** rusty_claw-91n
**Title:** Implement Control Protocol handler
**Priority:** P1 (Critical)
**Status:** in_progress

**Description:** Implement ControlProtocol struct with request/response routing, pending request tracking via oneshot channels, handler registration for can_use_tool/hook_callbacks/mcp_message, and the initialization handshake sequence.

**Dependencies (All Completed ✓):**
- ✓ rusty_claw-6cn: Implement Transport trait and SubprocessCLITransport [P1]
- ✓ rusty_claw-dss: Implement ClaudeAgentOptions builder [P2]

**Blocks (Downstream Tasks):**
- ○ rusty_claw-bip: Implement Hook system [P2]
- ○ rusty_claw-qrl: Implement ClaudeClient for interactive sessions [P2]
- ○ rusty_claw-tlh: Implement SDK MCP Server bridge [P2]

## Current State Analysis

### What Exists

**1. Transport Layer (✓ Complete)**
- `src/transport/mod.rs` - Transport trait with async interface
- `src/transport/subprocess.rs` - SubprocessCLITransport implementation
- `src/transport/discovery.rs` - CLI discovery and version check
- Full bidirectional NDJSON communication over stdio
- Message receiver via `messages()` returning `UnboundedReceiver<Result<Value, ClawError>>`

**2. Message Types (✓ Partial)**
- `src/messages.rs` - Core message types:
  - `Message` enum (System, Assistant, User, Result)
  - `SystemMessage` enum (Init, CompactBoundary)
  - `AssistantMessage`, `UserMessage`, `ResultMessage`
  - `ContentBlock` enum (Text, ToolUse, ToolResult, Thinking)
- ❌ **Missing:** Control protocol message types (`ControlRequest`, `ControlResponse`)

**3. Error Types (✓ Complete)**
- `src/error.rs` - ClawError enum with variants:
  - `CliNotFound`, `InvalidCliVersion`, `Connection`, `Process`
  - `JsonDecode`, `MessageParse`, `ControlTimeout`, `ControlError`
  - `Io`, `ToolExecution`
- All error types needed for control protocol already exist

**4. Options (✓ Complete)**
- `src/options.rs` - ClaudeAgentOptions with builder pattern
  - System prompt, max_turns, model, tools, permissions
  - MCP servers, hooks, agents (placeholder types)
  - Session, environment, output settings
  - `to_cli_args()` method for CLI argument conversion

**5. Control Module (❌ Missing)**
- `src/lib.rs` declares `pub mod control` as empty placeholder
- ❌ **No implementation files exist**

### What's Missing

**Critical Missing Files:**

1. **`src/control/mod.rs`** (NEW FILE, ~200 lines)
   - `ControlProtocol` struct
   - Core request/response routing
   - Module structure and re-exports

2. **`src/control/messages.rs`** (NEW FILE, ~300 lines)
   - `ControlRequest` enum (outgoing: initialize, interrupt, set_permission_mode, etc.)
   - `ControlResponse` enum (success, error responses)
   - `IncomingControlRequest` enum (incoming: can_use_tool, hook_callback, mcp_message)
   - Serde serialization/deserialization

3. **`src/control/handlers.rs`** (NEW FILE, ~150 lines)
   - `ControlHandlers` struct
   - Handler traits: `CanUseToolHandler`, `HookHandler`, `McpMessageHandler`
   - Handler registration system

4. **`src/control/pending.rs`** (NEW FILE, ~100 lines)
   - Pending request tracking with oneshot channels
   - Request ID generation (UUID)
   - Timeout handling

**Minor Missing Pieces:**

5. **`src/messages.rs`** (MODIFY, +30 lines)
   - Add `ControlRequest` and `ControlResponse` variants to `Message` enum
   - Update parsing logic

6. **`src/lib.rs`** (MODIFY, +5 lines)
   - Replace empty `control` module with `pub mod control;`
   - Update prelude exports

## Design Analysis

### Architecture Overview (from SPEC.md Section 4)

```
┌─────────────────────────────────────────────────────────────┐
│                      ControlProtocol                        │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐  │
│  │               Request/Response Router                │  │
│  │  - request() sends and awaits response              │  │
│  │  - handle_incoming() routes to handlers             │  │
│  └─────────────────────────────────────────────────────┘  │
│                          ↕                                  │
│  ┌──────────────────────┐      ┌─────────────────────┐   │
│  │  Pending Requests     │      │   Handlers          │   │
│  │  HashMap<String,      │      │   CanUseTool        │   │
│  │    oneshot::Sender>   │      │   HookCallbacks     │   │
│  │                       │      │   McpMessage        │   │
│  └──────────────────────┘      └─────────────────────┘   │
│                          ↕                                  │
│  ┌─────────────────────────────────────────────────────┐  │
│  │              Transport (Arc<dyn Transport>)         │  │
│  │  - write() sends messages to CLI stdin             │  │
│  │  - messages() receives from CLI stdout             │  │
│  └─────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### Key Design Patterns

**1. Request/Response Routing (SPEC.md 4.2)**
- Outgoing: `request()` generates UUID, inserts oneshot sender, writes message
- Incoming: Background task reads messages, routes `control_response` to pending sender
- Timeout: 30-second default with `tokio::time::timeout`

**2. Pending Request Tracking**
```rust
pending: Arc<Mutex<HashMap<String, oneshot::Sender<ControlResponse>>>>
```
- Key: request_id (UUID string)
- Value: oneshot sender for response
- Cleanup: Sender dropped on timeout or error

**3. Handler Registration**
```rust
struct ControlHandlers {
    can_use_tool: Option<Box<dyn CanUseToolHandler>>,
    hook_callbacks: HashMap<String, Box<dyn HookHandler>>,
    mcp_message: Option<Box<dyn McpMessageHandler>>,
}
```
- Dynamic registration via `register_*()` methods
- Async trait handlers for extensibility
- Option/HashMap for optional handlers

**4. Initialization Handshake (SPEC.md 4.4)**
```
1. SDK spawns CLI with --input-format=stream-json
2. SDK sends: control_request { subtype: "initialize", hooks: {...}, agents: {...}, ... }
3. CLI responds: control_response { subtype: "success", ... }
4. CLI sends: system message { subtype: "init", session_id: "...", tools: [...] }
5. SDK sends prompt via stdin (or control_request for client mode)
6. CLI begins processing and streaming messages back
```

### Message Flow

**Outgoing Request (SDK → CLI):**
```rust
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
```

**Incoming Request (CLI → SDK):**
```rust
pub async fn handle_incoming(&self, request_id: &str, request: IncomingControlRequest) {
    let response = match request {
        IncomingControlRequest::CanUseTool { tool_name, tool_input } => {
            if let Some(handler) = &self.handlers.lock().await.can_use_tool {
                handler.can_use_tool(&tool_name, &tool_input).await
            } else {
                // Default: allow all tools
                Ok(ControlResponse::Success { ... })
            }
        }
        // ... other handlers
    };

    let msg = json!({
        "type": "control_response",
        "request_id": request_id,
        "response": response,
    });
    self.transport.write(serde_json::to_vec(&msg)?.as_slice()).await?;
}
```

## Implementation Plan

### Phase 1: Control Message Types (~90 minutes)

**File: `src/control/messages.rs`**

1. **Outgoing Control Requests (SDK → CLI)**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum ControlRequest {
    Initialize {
        #[serde(skip_serializing_if = "HashMap::is_empty")]
        hooks: HashMap<HookEvent, Vec<HookMatcher>>,
        #[serde(skip_serializing_if = "HashMap::is_empty")]
        agents: HashMap<String, AgentDefinition>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        sdk_mcp_servers: Vec<SdkMcpServer>,
        #[serde(skip_serializing_if = "Option::is_none")]
        permissions: Option<PermissionMode>,
        can_use_tool: bool,
    },
    Interrupt,
    SetPermissionMode { mode: String },
    SetModel { model: String },
    McpStatus,
    RewindFiles { message_id: String },
}
```

2. **Control Responses (CLI → SDK, SDK → CLI)**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum ControlResponse {
    Success {
        #[serde(flatten)]
        data: serde_json::Value,
    },
    Error {
        error: String,
        #[serde(flatten)]
        extra: serde_json::Value,
    },
}
```

3. **Incoming Control Requests (CLI → SDK)**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum IncomingControlRequest {
    CanUseTool {
        tool_name: String,
        tool_input: serde_json::Value,
    },
    HookCallback {
        hook_id: String,
        hook_event: HookEvent,
        hook_input: serde_json::Value,
    },
    McpMessage {
        server_name: String,
        message: serde_json::Value, // JSON-RPC
    },
}
```

**Tests:**
- Serialization/deserialization for all message types
- Round-trip tests (serialize → deserialize → compare)
- Edge cases (empty fields, missing optionals)

### Phase 2: Handler Traits and Registration (~60 minutes)

**File: `src/control/handlers.rs`**

1. **Handler Traits**
```rust
#[async_trait]
pub trait CanUseToolHandler: Send + Sync {
    async fn can_use_tool(
        &self,
        tool_name: &str,
        tool_input: &serde_json::Value,
    ) -> Result<bool, ClawError>;
}

#[async_trait]
pub trait HookHandler: Send + Sync {
    async fn call(
        &self,
        hook_event: HookEvent,
        hook_input: serde_json::Value,
    ) -> Result<serde_json::Value, ClawError>;
}

#[async_trait]
pub trait McpMessageHandler: Send + Sync {
    async fn handle(
        &self,
        server_name: &str,
        message: serde_json::Value,
    ) -> Result<serde_json::Value, ClawError>;
}
```

2. **Handler Registry**
```rust
pub struct ControlHandlers {
    can_use_tool: Option<Arc<dyn CanUseToolHandler>>,
    hook_callbacks: HashMap<String, Arc<dyn HookHandler>>,
    mcp_message: Option<Arc<dyn McpMessageHandler>>,
}

impl ControlHandlers {
    pub fn new() -> Self { ... }
    pub fn register_can_use_tool(&mut self, handler: Arc<dyn CanUseToolHandler>) { ... }
    pub fn register_hook(&mut self, hook_id: String, handler: Arc<dyn HookHandler>) { ... }
    pub fn register_mcp_message(&mut self, handler: Arc<dyn McpMessageHandler>) { ... }
}
```

**Tests:**
- Mock handlers for testing
- Registration and retrieval
- Multiple hook registration

### Phase 3: Pending Request Tracking (~45 minutes)

**File: `src/control/pending.rs`**

1. **Pending Request Manager**
```rust
pub struct PendingRequests {
    inner: Arc<Mutex<HashMap<String, oneshot::Sender<ControlResponse>>>>,
}

impl PendingRequests {
    pub fn new() -> Self { ... }

    pub async fn insert(&self, id: String, sender: oneshot::Sender<ControlResponse>) {
        self.inner.lock().await.insert(id, sender);
    }

    pub async fn complete(&self, id: &str, response: ControlResponse) -> bool {
        if let Some(sender) = self.inner.lock().await.remove(id) {
            sender.send(response).is_ok()
        } else {
            false
        }
    }

    pub async fn cancel(&self, id: &str) {
        self.inner.lock().await.remove(id);
    }
}
```

**Tests:**
- Insert and complete
- Cancel (timeout simulation)
- Multiple pending requests
- Concurrent access

### Phase 4: ControlProtocol Core (~90 minutes)

**File: `src/control/mod.rs`**

1. **Main Struct**
```rust
pub struct ControlProtocol {
    transport: Arc<dyn Transport>,
    pending: PendingRequests,
    handlers: Arc<Mutex<ControlHandlers>>,
}

impl ControlProtocol {
    pub fn new(transport: Arc<dyn Transport>) -> Self {
        Self {
            transport,
            pending: PendingRequests::new(),
            handlers: Arc::new(Mutex::new(ControlHandlers::new())),
        }
    }
}
```

2. **Request Method (SDK → CLI)**
```rust
pub async fn request(&self, request: ControlRequest) -> Result<ControlResponse, ClawError> {
    let id = Uuid::new_v4().to_string();
    let (tx, rx) = oneshot::channel();
    self.pending.insert(id.clone(), tx).await;

    let msg = json!({
        "type": "control_request",
        "request_id": id,
        "request": request,
    });
    self.transport.write(serde_json::to_vec(&msg)?.as_slice()).await?;

    match tokio::time::timeout(Duration::from_secs(30), rx).await {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(_)) => Err(ClawError::ControlError("channel closed".to_string())),
        Err(_) => {
            self.pending.cancel(&id).await;
            Err(ClawError::ControlTimeout { subtype: "control_request".to_string() })
        }
    }
}
```

3. **Response Handler (CLI → SDK)**
```rust
pub async fn handle_response(&self, request_id: &str, response: ControlResponse) {
    self.pending.complete(request_id, response).await;
}
```

4. **Incoming Request Handler (CLI → SDK)**
```rust
pub async fn handle_incoming(&self, request_id: &str, request: IncomingControlRequest) {
    let response = match request {
        IncomingControlRequest::CanUseTool { tool_name, tool_input } => {
            let handlers = self.handlers.lock().await;
            if let Some(handler) = &handlers.can_use_tool {
                match handler.can_use_tool(&tool_name, &tool_input).await {
                    Ok(allowed) => ControlResponse::Success {
                        data: json!({ "allowed": allowed }),
                    },
                    Err(e) => ControlResponse::Error {
                        error: e.to_string(),
                        extra: json!({}),
                    },
                }
            } else {
                // Default: allow all tools
                ControlResponse::Success {
                    data: json!({ "allowed": true }),
                }
            }
        }
        IncomingControlRequest::HookCallback { hook_id, hook_event, hook_input } => {
            let handlers = self.handlers.lock().await;
            if let Some(handler) = handlers.hook_callbacks.get(&hook_id) {
                match handler.call(hook_event, hook_input).await {
                    Ok(result) => ControlResponse::Success { data: result },
                    Err(e) => ControlResponse::Error {
                        error: e.to_string(),
                        extra: json!({}),
                    },
                }
            } else {
                ControlResponse::Error {
                    error: format!("No handler for hook_id: {}", hook_id),
                    extra: json!({}),
                }
            }
        }
        IncomingControlRequest::McpMessage { server_name, message } => {
            let handlers = self.handlers.lock().await;
            if let Some(handler) = &handlers.mcp_message {
                match handler.handle(&server_name, message).await {
                    Ok(result) => ControlResponse::Success { data: result },
                    Err(e) => ControlResponse::Error {
                        error: e.to_string(),
                        extra: json!({}),
                    },
                }
            } else {
                ControlResponse::Error {
                    error: format!("No MCP message handler registered"),
                    extra: json!({}),
                }
            }
        }
    };

    let msg = json!({
        "type": "control_response",
        "request_id": request_id,
        "response": response,
    });
    if let Err(e) = self.transport.write(serde_json::to_vec(&msg).unwrap().as_slice()).await {
        eprintln!("Failed to send control response: {}", e);
    }
}
```

**Tests:**
- Request/response round-trip with mock transport
- Timeout handling
- Concurrent requests
- Handler dispatch

### Phase 5: Initialization Handshake (~60 minutes)

**File: `src/control/mod.rs` (additions)**

```rust
impl ControlProtocol {
    pub async fn initialize(&self, options: &ClaudeAgentOptions) -> Result<(), ClawError> {
        let request = ControlRequest::Initialize {
            hooks: options.hooks.clone(),
            agents: options.agents.clone(),
            sdk_mcp_servers: options.sdk_mcp_servers.clone(),
            permissions: options.permission_mode.clone(),
            can_use_tool: true, // Enable can_use_tool callbacks
        };

        match self.request(request).await? {
            ControlResponse::Success { .. } => Ok(()),
            ControlResponse::Error { error, .. } => {
                Err(ClawError::ControlError(format!("Initialization failed: {}", error)))
            }
        }
    }
}
```

**Tests:**
- Successful initialization
- Initialization failure handling
- Options conversion

### Phase 6: Message Integration (~45 minutes)

**File: `src/messages.rs` (modifications)**

1. **Add Control Message Variants**
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
}
```

**Tests:**
- Parsing control_request and control_response messages
- Round-trip serialization

### Phase 7: Integration Tests (~60 minutes)

**File: `src/control/tests.rs`**

1. **Mock Transport**
```rust
struct MockTransport {
    sent: Arc<Mutex<Vec<String>>>,
    receiver: Option<mpsc::UnboundedReceiver<Result<Value, ClawError>>>,
    sender: mpsc::UnboundedSender<Result<Value, ClawError>>,
}
```

2. **Full Flow Tests**
- Initialize handshake
- Request/response with mock CLI
- Incoming request handling
- Error scenarios (timeout, parse error, handler error)
- Concurrent request handling

### Phase 8: Documentation & Polish (~30 minutes)

1. **Module Documentation**
   - Overview and architecture
   - Usage examples
   - Handler registration examples

2. **Public API Documentation**
   - All public methods
   - Error conditions
   - Thread safety guarantees

## Files to Modify/Create

### New Files (4 files, ~750 lines total)

1. **`src/control/mod.rs`** (~200 lines)
   - ControlProtocol struct
   - request(), handle_response(), handle_incoming() methods
   - initialize() handshake method
   - Module structure and re-exports

2. **`src/control/messages.rs`** (~300 lines)
   - ControlRequest enum (6 variants)
   - ControlResponse enum (2 variants)
   - IncomingControlRequest enum (3 variants)
   - Serde implementations
   - Tests (~100 lines)

3. **`src/control/handlers.rs`** (~150 lines)
   - CanUseToolHandler trait
   - HookHandler trait
   - McpMessageHandler trait
   - ControlHandlers registry struct
   - Tests (~50 lines)

4. **`src/control/pending.rs`** (~100 lines)
   - PendingRequests struct
   - insert(), complete(), cancel() methods
   - Tests (~30 lines)

### Modified Files (3 files, ~50 lines changed)

5. **`src/lib.rs`** (~5 lines changed)
   - Replace empty `control` module with `pub mod control;`
   - Update prelude exports:
     ```rust
     pub use crate::control::{ControlProtocol, ControlRequest, ControlResponse};
     ```

6. **`src/messages.rs`** (~30 lines changed)
   - Add ControlRequest and ControlResponse variants to Message enum
   - Update parsing and serialization tests

7. **`Cargo.toml`** (~15 lines changed)
   - Add dependencies:
     - `uuid = { version = "1.11", features = ["v4", "serde"] }`
     - (other dependencies already present: tokio, serde, serde_json, async-trait)

## Dependencies

### New Dependencies Required

```toml
[dependencies]
uuid = { version = "1.11", features = ["v4", "serde"] }
```

### Existing Dependencies (Already Available)

- `tokio` - async runtime, channels (oneshot, mpsc)
- `serde` / `serde_json` - serialization
- `async-trait` - async trait support
- `thiserror` - error handling

## Risks & Mitigations

### 🟢 Low Risk

1. **Message Serialization**
   - Risk: JSON format mismatch with CLI
   - Mitigation: Comprehensive unit tests, follow SPEC.md exactly
   - Testing: Round-trip ser/de tests for all message types

2. **Handler Traits**
   - Risk: Trait design too rigid or too flexible
   - Mitigation: Follow Python SDK patterns, async-trait for flexibility
   - Testing: Mock handlers in tests

### 🟡 Medium Risk

1. **Pending Request Cleanup**
   - Risk: Memory leak from abandoned requests
   - Mitigation: Timeout removes pending entry, explicit cancel on drop
   - Testing: Timeout tests, concurrent request tests

2. **Handler Dispatch Errors**
   - Risk: Unhandled errors crash process
   - Mitigation: Catch all errors, return ControlResponse::Error
   - Testing: Error propagation tests

3. **Concurrency Issues**
   - Risk: Race conditions in pending/handlers maps
   - Mitigation: Arc<Mutex<...>> for all shared state
   - Testing: Concurrent access tests with tokio::spawn

### 🔴 High Risk

1. **Initialization Handshake Timing**
   - Risk: Race between initialize request and system init message
   - Mitigation: Wait for ControlResponse::Success before proceeding
   - Testing: Integration tests with mock transport, sequence verification

2. **Backward Compatibility**
   - Risk: Breaking changes to Message enum affect existing code
   - Mitigation: Add variants, don't modify existing ones
   - Testing: All 73 existing tests must pass

## Testing Strategy

### Unit Tests (~200 lines)

**Messages (`control/messages.rs`):**
- ✅ Serialize/deserialize ControlRequest variants
- ✅ Serialize/deserialize ControlResponse variants
- ✅ Serialize/deserialize IncomingControlRequest variants
- ✅ Round-trip tests
- ✅ Missing optional fields

**Handlers (`control/handlers.rs`):**
- ✅ Mock handler implementations
- ✅ Handler registration
- ✅ Multiple hooks registration

**Pending (`control/pending.rs`):**
- ✅ Insert and complete
- ✅ Cancel (timeout)
- ✅ Concurrent access

### Integration Tests (~150 lines)

**Control Protocol (`control/mod.rs`):**
- ✅ Request/response with mock transport
- ✅ Timeout handling
- ✅ Concurrent requests
- ✅ Handler dispatch (can_use_tool, hook, mcp)
- ✅ Initialize handshake

**Message Integration (`messages.rs`):**
- ✅ Parse control_request from JSON
- ✅ Parse control_response from JSON
- ✅ No regressions in existing 73 tests

### Success Criteria

1. ✅ All 4 new files created with complete implementations
2. ✅ All 3 modified files updated correctly
3. ✅ ControlProtocol struct with request/response routing
4. ✅ Pending request tracking with oneshot channels
5. ✅ Handler traits and registration system
6. ✅ Initialization handshake method
7. ✅ Message enum updated with control variants
8. ✅ All unit tests pass (new: ~20 tests)
9. ✅ All integration tests pass (new: ~10 tests)
10. ✅ All existing tests pass (73 tests)
11. ✅ Zero clippy warnings in new code
12. ✅ Complete module documentation
13. ✅ No breaking changes to existing API

## Downstream Impact

**Unblocks 3 Critical Tasks:**

1. **rusty_claw-bip** [P2] - Implement Hook system
   - Needs: `ControlHandlers`, `HookHandler` trait
   - Can implement hook registration and callbacks

2. **rusty_claw-qrl** [P2] - Implement ClaudeClient for interactive sessions
   - Needs: `ControlProtocol`, `initialize()` method
   - Can start interactive sessions with proper handshake

3. **rusty_claw-tlh** [P2] - Implement SDK MCP Server bridge
   - Needs: `McpMessageHandler` trait, message routing
   - Can route JSON-RPC messages to SDK-hosted tools

## Implementation Checklist

### Phase 1: Control Message Types ✓
- [ ] Create `src/control/messages.rs`
- [ ] Implement ControlRequest enum with 6 variants
- [ ] Implement ControlResponse enum with 2 variants
- [ ] Implement IncomingControlRequest enum with 3 variants
- [ ] Add serde derive macros with correct attributes
- [ ] Write serialization unit tests (8 tests)
- [ ] Write deserialization unit tests (8 tests)
- [ ] Write round-trip tests (3 tests)

### Phase 2: Handler Traits ✓
- [ ] Create `src/control/handlers.rs`
- [ ] Implement CanUseToolHandler trait
- [ ] Implement HookHandler trait
- [ ] Implement McpMessageHandler trait
- [ ] Implement ControlHandlers registry struct
- [ ] Add registration methods (3 methods)
- [ ] Write mock handler tests (5 tests)

### Phase 3: Pending Request Tracking ✓
- [ ] Create `src/control/pending.rs`
- [ ] Implement PendingRequests struct
- [ ] Implement insert() method
- [ ] Implement complete() method
- [ ] Implement cancel() method
- [ ] Write unit tests (5 tests)

### Phase 4: ControlProtocol Core ✓
- [ ] Create `src/control/mod.rs`
- [ ] Implement ControlProtocol struct
- [ ] Implement new() constructor
- [ ] Implement request() method (SDK → CLI)
- [ ] Implement handle_response() method (CLI → SDK)
- [ ] Implement handle_incoming() method (CLI → SDK)
- [ ] Add timeout handling (30 seconds)
- [ ] Write integration tests (8 tests)

### Phase 5: Initialization Handshake ✓
- [ ] Implement initialize() method in ControlProtocol
- [ ] Convert ClaudeAgentOptions to Initialize request
- [ ] Handle success/error responses
- [ ] Write initialization tests (3 tests)

### Phase 6: Message Integration ✓
- [ ] Modify `src/messages.rs`
- [ ] Add ControlRequest variant to Message enum
- [ ] Add ControlResponse variant to Message enum
- [ ] Update parsing logic
- [ ] Write integration tests (4 tests)
- [ ] Verify all 73 existing tests pass

### Phase 7: Integration Tests ✓
- [ ] Create MockTransport for testing
- [ ] Write full flow tests (5 tests)
- [ ] Write error scenario tests (5 tests)
- [ ] Write concurrent request tests (2 tests)

### Phase 8: Documentation & Polish ✓
- [ ] Write module-level documentation
- [ ] Document all public methods
- [ ] Add usage examples
- [ ] Add architecture diagram
- [ ] Run clippy and fix warnings
- [ ] Run cargo fmt
- [ ] Final test run (all ~103 tests)

### Phase 9: Verification ✓
- [ ] Cargo build succeeds
- [ ] Cargo test succeeds (all ~103 tests)
- [ ] Cargo clippy (0 warnings in new code)
- [ ] Cargo doc builds successfully
- [ ] Manual smoke test with mock transport
- [ ] Update lib.rs prelude exports
- [ ] Update Cargo.toml dependencies

## Estimated Timeline

- **Phase 1:** Control Message Types - 90 minutes
- **Phase 2:** Handler Traits - 60 minutes
- **Phase 3:** Pending Request Tracking - 45 minutes
- **Phase 4:** ControlProtocol Core - 90 minutes
- **Phase 5:** Initialization Handshake - 60 minutes
- **Phase 6:** Message Integration - 45 minutes
- **Phase 7:** Integration Tests - 60 minutes
- **Phase 8:** Documentation & Polish - 30 minutes
- **Phase 9:** Verification - 30 minutes

**Total: ~510 minutes (~8.5 hours)**

## Success Metrics

1. **Code Coverage:** 100% of public API
2. **Test Pass Rate:** 100% (new: ~30 tests, existing: 73 tests)
3. **Clippy Warnings:** 0 in new code
4. **Documentation:** Complete for all public items
5. **Downstream Unblocked:** 3 P2 tasks ready to start

## Notes

- Follow SPEC.md section 4 exactly for message formats
- Use Arc<Mutex<...>> for all shared mutable state
- Handler errors should not panic - return ControlResponse::Error
- Pending requests MUST be cleaned up on timeout
- All trait handlers are async for maximum flexibility
- Default behavior (no handler) should be permissive (allow tools, etc.)
- The control protocol is critical path - comprehensive testing required

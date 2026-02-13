# Investigation: rusty_claw-qrl - Implement ClaudeClient for interactive sessions

**Task ID:** rusty_claw-qrl
**Priority:** P2 (High)
**Status:** in_progress
**Date:** 2026-02-13

## Task Overview

Implement a **ClaudeClient** struct that maintains long-running interactive sessions with the Claude CLI. This client will:
- Manage session lifecycle (connect, configure, close)
- Send messages to Claude and receive streaming responses
- Support runtime control operations (interrupt, permission mode changes, model switching)
- Build on top of existing Control Protocol handler and query() function

## Dependencies Status

✅ **All dependencies satisfied:**
1. ✅ **rusty_claw-91n** - Control Protocol handler (COMPLETE)
2. ✅ **rusty_claw-sna** - query() function (COMPLETE)

## Existing Foundation (What We Have)

### 1. Control Protocol Infrastructure (rusty_claw-91n)

**Location:** `src/control/mod.rs`, `src/control/messages.rs`

**What it provides:**
- `ControlProtocol` struct - Manages bidirectional control communication
- Request/response routing with timeout handling
- Handler registration system (can_use_tool, hooks, MCP)
- Session initialization via `initialize()` method
- Control operations:
  - `Interrupt` - Stop current execution
  - `SetPermissionMode` - Change permission mode at runtime
  - `SetModel` - Switch Claude model at runtime
  - `McpStatus` - Query MCP server status
  - `RewindFiles` - Roll back filesystem changes

**Key methods:**
```rust
impl ControlProtocol {
    pub fn new(transport: Arc<dyn Transport>) -> Self
    pub async fn initialize(&self, options: &ClaudeAgentOptions) -> Result<(), ClawError>
    pub async fn request(&self, request: ControlRequest) -> Result<ControlResponse, ClawError>
    pub async fn handle_incoming(&self, request_id: &str, request: IncomingControlRequest)
    pub async fn handle_response(&self, request_id: &str, response: ControlResponse)
    pub async fn handlers(&self) -> MutexGuard<'_, ControlHandlers>
}
```

### 2. Query API (rusty_claw-sna)

**Location:** `src/query.rs`

**What it provides:**
- One-shot query interface via `query()` function
- Stream-based response handling
- `QueryStream<S>` wrapper that owns transport for lifetime management
- Automatic CLI discovery, connection, and message parsing

**Pattern to follow:**
```rust
pub async fn query(
    prompt: impl Into<String>,
    options: Option<ClaudeAgentOptions>,
) -> Result<impl Stream<Item = Result<Message, ClawError>>, ClawError>
```

**Key insight:** QueryStream solves the lifetime management problem by owning the transport. ClaudeClient will need similar approach.

### 3. Transport Layer

**Location:** `src/transport/mod.rs`, `src/transport/subprocess.rs`

**What it provides:**
- `Transport` trait - Abstract interface for CLI communication
- `SubprocessCLITransport` - Subprocess implementation
- Bidirectional communication (stdin writes, stdout reads)
- NDJSON message framing and parsing
- Process lifecycle management

**Key methods:**
```rust
#[async_trait]
pub trait Transport: Send + Sync {
    async fn connect(&mut self) -> Result<(), ClawError>;
    async fn write(&self, message: &[u8]) -> Result<(), ClawError>;
    fn messages(&self) -> mpsc::UnboundedReceiver<Result<Value, ClawError>>;
    async fn end_input(&self) -> Result<(), ClawError>;
    async fn close(&mut self) -> Result<(), ClawError>;
    fn is_ready(&self) -> bool;
}
```

### 4. Message Types

**Location:** `src/messages.rs`

**Available message types:**
- `Message::System(SystemMessage)` - Init, CompactBoundary
- `Message::Assistant(AssistantMessage)` - Claude responses with content blocks
- `Message::User(UserMessage)` - User input messages
- `Message::Result(ResultMessage)` - Success, Error, InputRequired
- `Message::ControlRequest` - Control protocol requests
- `Message::ControlResponse` - Control protocol responses

**Content blocks:**
- `ContentBlock::Text` - Plain text content
- `ContentBlock::ToolUse` - Tool invocation requests
- `ContentBlock::ToolResult` - Tool execution results
- `ContentBlock::Thinking` - Extended thinking tokens

**Streaming:**
- `StreamEvent` struct exists for streaming delta updates

### 5. Configuration Options

**Location:** `src/options.rs`

**ClaudeAgentOptions provides:**
- System prompt configuration
- Model selection
- Permission mode
- Hook configuration
- Agent definitions
- MCP server configuration
- Max turns, turn cost limits
- Tool allowlists/denylists
- CLI arg generation via `to_cli_args()`

## What Needs to Be Implemented

### 1. ClaudeClient Struct

**New module:** `src/client.rs` or `src/client/mod.rs`

**Core structure:**
```rust
pub struct ClaudeClient {
    // Control protocol for sending requests and handling incoming
    control: Arc<ControlProtocol>,

    // Transport for message I/O
    transport: Arc<dyn Transport>,

    // Session configuration
    options: ClaudeAgentOptions,

    // Session state
    session_id: Option<String>,
    is_initialized: Arc<Mutex<bool>>,

    // Message receiver for streaming responses
    // Option because messages() can only be called once
    message_rx: Arc<Mutex<Option<mpsc::UnboundedReceiver<Result<Value, ClawError>>>>>,
}
```

**Thread safety:** All fields must be thread-safe (Send + Sync) for async use.

### 2. Session Management

**Methods to implement:**

```rust
impl ClaudeClient {
    /// Create a new client (does not connect)
    pub fn new(options: ClaudeAgentOptions) -> Result<Self, ClawError>;

    /// Connect to CLI and initialize session
    pub async fn connect(&mut self) -> Result<(), ClawError>;

    /// Check if client is connected and ready
    pub fn is_connected(&self) -> bool;

    /// Close the session gracefully
    pub async fn close(&mut self) -> Result<(), ClawError>;
}
```

**Flow:**
1. `new()` - Create client with options (transport not created yet)
2. `connect()` - Create transport, connect, initialize control protocol
3. Session is ready for message sending
4. `close()` - End input, wait for CLI exit, cleanup

### 3. Message Sending

**Methods to implement:**

```rust
impl ClaudeClient {
    /// Send a message and return a stream of responses
    pub async fn send_message(
        &self,
        content: impl Into<String>,
    ) -> Result<ResponseStream, ClawError>;

    /// Internal: Write a message to CLI stdin
    async fn write_message(&self, content: &str) -> Result<(), ClawError>;
}
```

**Message format:**
```json
{
  "type": "user",
  "message": {
    "role": "user",
    "content": [{"type": "text", "text": "..."}]
  }
}
```

**ResponseStream design:**
- Wrap the message receiver from transport
- Parse raw `Value` into typed `Message` structs
- Handle control protocol messages internally (route to handlers)
- Yield only user-facing messages (Assistant, Result, System)
- Support async iteration via `Stream` trait

### 4. Streaming Response Handling

**New type:**
```rust
pub struct ResponseStream {
    // Receiver for raw messages
    rx: mpsc::UnboundedReceiver<Result<Value, ClawError>>,

    // Control protocol reference for routing control messages
    control: Arc<ControlProtocol>,

    // Stream state tracking
    is_complete: bool,
}
```

**Stream implementation:**
```rust
impl Stream for ResponseStream {
    type Item = Result<Message, ClawError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>)
        -> Poll<Option<Self::Item>>;
}
```

**Message routing logic:**
- `Message::ControlRequest` → Route to `control.handle_incoming()`
- `Message::ControlResponse` → Route to `control.handle_response()`
- `Message::Assistant/User/Result/System` → Yield to user
- End of stream → Set `is_complete = true`

### 5. Session Control Operations

**Methods to implement:**

```rust
impl ClaudeClient {
    /// Interrupt the current agent execution
    pub async fn interrupt(&self) -> Result<(), ClawError>;

    /// Change permission mode during session
    pub async fn set_permission_mode(&self, mode: PermissionMode) -> Result<(), ClawError>;

    /// Switch the active model during session
    pub async fn set_model(&self, model: impl Into<String>) -> Result<(), ClawError>;

    /// Query MCP server connection status
    pub async fn mcp_status(&self) -> Result<serde_json::Value, ClawError>;

    /// Rewind file state to a specific message
    pub async fn rewind_files(&self, message_id: impl Into<String>) -> Result<(), ClawError>;
}
```

**Implementation pattern:**
```rust
pub async fn interrupt(&self) -> Result<(), ClawError> {
    let response = self.control.request(ControlRequest::Interrupt).await?;
    match response {
        ControlResponse::Success { .. } => Ok(()),
        ControlResponse::Error { error, .. } => {
            Err(ClawError::ControlError(format!("Interrupt failed: {}", error)))
        }
    }
}
```

### 6. Handler Registration

**Methods to implement:**

```rust
impl ClaudeClient {
    /// Register a handler for can_use_tool requests
    pub async fn register_can_use_tool_handler(
        &self,
        handler: Arc<dyn CanUseToolHandler>,
    );

    /// Register a hook callback
    pub async fn register_hook(
        &self,
        hook_id: String,
        handler: Arc<dyn HookHandler>,
    );

    /// Register an MCP message handler
    pub async fn register_mcp_message_handler(
        &self,
        handler: Arc<dyn McpMessageHandler>,
    );
}
```

**Implementation:** Delegate to `control.handlers()` and register on the ControlHandlers instance.

## Architecture Design

### Ownership and Lifetimes

**Problem:** Transport's `messages()` method can only be called once (consumes the receiver).

**Solution (inspired by QueryStream):**
1. ClaudeClient stores `Option<Receiver>` in Arc<Mutex<>>
2. On `connect()`, call `transport.messages()` once and store
3. `send_message()` takes receiver out of Option, wraps in ResponseStream
4. ResponseStream owns the receiver for its lifetime
5. When ResponseStream is dropped, client can't send more messages (session over)

**Alternative approach:** Multiple message sending
- Store receiver permanently in ClaudeClient
- `send_message()` doesn't take ownership, just yields from shared receiver
- All messages from CLI are routed through a single stream
- Client needs internal task to continuously process messages

**Recommendation:** Start with single-message approach (simpler), can extend later.

### Message Routing

```
CLI stdout → Transport → UnboundedReceiver<Result<Value>>
                              ↓
                         ClaudeClient (routing)
                              ↓
                    ┌─────────┴──────────┐
                    ↓                    ↓
         Control Messages         User Messages
         (ControlRequest,         (Assistant,
          ControlResponse)         Result, etc.)
                    ↓                    ↓
            ControlProtocol          ResponseStream
            (handle internally)      (yield to user)
```

### Concurrent Operations

**Thread safety requirements:**
- Multiple tasks can call control operations concurrently
- ControlProtocol already handles concurrent requests (Arc + Mutex)
- ResponseStream is single-consumer (one task polls it)
- Transport write is thread-safe (Arc<dyn Transport>)

## Files to Create

### Primary Implementation

**File:** `crates/rusty_claw/src/client.rs` (~500-700 lines)

**Structure:**
```rust
//! ClaudeClient for interactive sessions with Claude CLI
//!
//! # Overview
//! ...
//!
//! # Example
//! ...

use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_stream::Stream;

// ClaudeClient struct
pub struct ClaudeClient { /* ... */ }

// Session management methods
impl ClaudeClient {
    pub fn new() { /* ... */ }
    pub async fn connect() { /* ... */ }
    pub fn is_connected() { /* ... */ }
    pub async fn close() { /* ... */ }
}

// Message sending methods
impl ClaudeClient {
    pub async fn send_message() { /* ... */ }
    async fn write_message() { /* ... */ }
}

// Control operations
impl ClaudeClient {
    pub async fn interrupt() { /* ... */ }
    pub async fn set_permission_mode() { /* ... */ }
    pub async fn set_model() { /* ... */ }
    pub async fn mcp_status() { /* ... */ }
    pub async fn rewind_files() { /* ... */ }
}

// Handler registration
impl ClaudeClient {
    pub async fn register_can_use_tool_handler() { /* ... */ }
    pub async fn register_hook() { /* ... */ }
    pub async fn register_mcp_message_handler() { /* ... */ }
}

// ResponseStream struct
pub struct ResponseStream { /* ... */ }

impl Stream for ResponseStream {
    type Item = Result<Message, ClawError>;
    fn poll_next() { /* ... */ }
}

// Tests module
#[cfg(test)]
mod tests { /* ... */ }
```

### Modified Files

**File:** `crates/rusty_claw/src/lib.rs` (+3 lines)

Add module declaration and prelude export:
```rust
// Add after query module
pub mod client;

// Update prelude
pub use crate::client::{ClaudeClient, ResponseStream};
```

## Implementation Plan

### Phase 1: Basic Structure (60 min)

**Tasks:**
1. Create `src/client.rs` with module docs
2. Define `ClaudeClient` struct with all fields
3. Define `ResponseStream` struct
4. Implement `ClaudeClient::new()` (no connection yet)
5. Implement `is_connected()` (check transport.is_ready())
6. Add module to lib.rs and prelude

**Deliverable:** Compiles, struct is public and documented.

### Phase 2: Session Management (90 min)

**Tasks:**
1. Implement `ClaudeClient::connect()`
   - Create SubprocessCLITransport
   - Call transport.connect()
   - Extract CLI args from options
   - Create ControlProtocol
   - Call control.initialize()
   - Store message receiver in Arc<Mutex<Option<>>>
2. Implement `ClaudeClient::close()`
   - Call transport.end_input()
   - Call transport.close()
   - Set is_initialized to false
3. Add basic unit tests for lifecycle

**Deliverable:** Can create, connect, and close a client.

### Phase 3: Message Sending (90 min)

**Tasks:**
1. Implement `write_message()` helper
   - Format user message JSON
   - Serialize to bytes
   - Call transport.write()
2. Implement `send_message()`
   - Check is_connected
   - Call write_message()
   - Take receiver from Arc<Mutex<Option<>>>
   - Wrap in ResponseStream
   - Return ResponseStream
3. Add tests for message formatting

**Deliverable:** Can send messages and get response stream.

### Phase 4: Streaming Response Handling (120 min)

**Tasks:**
1. Implement `ResponseStream` struct
   - Store receiver and control reference
   - Add is_complete flag
2. Implement `Stream` trait for ResponseStream
   - Poll receiver for next message
   - Parse Value into Message
   - Route control messages to ControlProtocol
   - Yield user-facing messages
   - Handle end of stream
3. Add helper methods (is_complete(), etc.)
4. Add comprehensive streaming tests
   - Mock transport with canned responses
   - Test control message routing
   - Test user message yielding
   - Test end of stream handling

**Deliverable:** ResponseStream works correctly with message routing.

### Phase 5: Control Operations (60 min)

**Tasks:**
1. Implement `interrupt()`
2. Implement `set_permission_mode()`
3. Implement `set_model()`
4. Implement `mcp_status()`
5. Implement `rewind_files()`
6. Add unit tests for each operation

**Deliverable:** All control operations work.

### Phase 6: Handler Registration (30 min)

**Tasks:**
1. Implement `register_can_use_tool_handler()`
2. Implement `register_hook()`
3. Implement `register_mcp_message_handler()`
4. Add tests for handler registration

**Deliverable:** Handlers can be registered.

### Phase 7: Integration Tests (90 min)

**Tasks:**
1. Create MockTransport for integration testing
2. Test full session lifecycle (connect → send → receive → close)
3. Test concurrent control operations
4. Test error scenarios (connection failure, CLI error responses)
5. Test handler invocation during streaming
6. Test interrupted streams
7. Test permission mode changes during session

**Deliverable:** ~20-30 comprehensive tests.

### Phase 8: Documentation & Polish (60 min)

**Tasks:**
1. Write comprehensive module-level docs with examples
2. Document all public structs, methods, and types
3. Add usage examples showing:
   - Basic session
   - Streaming responses
   - Control operations
   - Handler registration
4. Add doctests for key examples
5. Run clippy and fix any warnings
6. Run cargo doc and verify rendering
7. Update README/docs with ClaudeClient info

**Deliverable:** 100% documentation coverage, zero clippy warnings.

### Phase 9: Final Verification (30 min)

**Tasks:**
1. Run full test suite (cargo test)
2. Verify no regressions in existing tests
3. Check test coverage (aim for >90%)
4. Run clippy in CI mode (treat warnings as errors)
5. Verify thread safety (Send + Sync bounds)
6. Test compile times (should be reasonable)
7. Create final summary

**Deliverable:** Production-ready implementation.

## Expected Outcomes

### New Files (1 file, ~600-800 lines)

1. **`crates/rusty_claw/src/client.rs`** (~600-800 lines)
   - ClaudeClient struct (~100 lines)
   - Session management methods (~150 lines)
   - Message sending methods (~100 lines)
   - Control operations (~100 lines)
   - Handler registration (~50 lines)
   - ResponseStream struct + Stream impl (~100 lines)
   - Tests module (~200-300 lines)

### Modified Files (1 file, +5 lines)

2. **`crates/rusty_claw/src/lib.rs`** (+5 lines)
   - Add `pub mod client;`
   - Update prelude with `ClaudeClient` and `ResponseStream`

### Test Coverage

**Target:** 20-30 comprehensive tests
- Unit tests: ~15 tests (lifecycle, message formatting, control ops)
- Integration tests: ~10 tests (full session, concurrent ops, error handling)
- Doctests: ~5 tests (usage examples)

**Categories:**
- Session lifecycle (connect, close, reconnect)
- Message sending (single message, multiple messages)
- Response streaming (delta updates, end of stream)
- Control operations (interrupt, mode changes, model switch)
- Handler registration (can_use_tool, hooks, MCP)
- Error handling (connection failure, CLI errors, timeouts)
- Concurrent operations (multiple control requests)
- Thread safety (Send + Sync)

### Documentation

**Module-level docs:**
- Overview of ClaudeClient purpose
- Architecture explanation (vs query API)
- Basic usage example
- Advanced usage examples (streaming, control ops)

**Struct/method docs:**
- Complete API documentation
- Examples for all public methods
- Error documentation
- Thread safety notes

## Dependencies & Risks

### External Dependencies

**Already in Cargo.toml:**
- `tokio` - Async runtime (Mutex, mpsc channels)
- `tokio-stream` - Stream trait and utilities
- `async-trait` - Async trait methods
- `serde`, `serde_json` - Serialization
- `uuid` - Request ID generation (via ControlProtocol)

**No new dependencies needed.**

### Integration Risks

**Risk:** Message receiver can only be called once.
**Mitigation:** Store in `Arc<Mutex<Option<>>>`, take out when needed. Document single-use behavior.

**Risk:** Control message routing complexity.
**Mitigation:** Route internally in ResponseStream.poll_next(). User never sees control messages.

**Risk:** Concurrent access to session state.
**Mitigation:** Use Arc<Mutex<>> for shared state. All operations are async-safe.

**Risk:** Stream lifetime and ownership.
**Mitigation:** Follow QueryStream pattern - ResponseStream owns what it needs.

### Breaking Changes

**None expected.** This is a new module with no impact on existing APIs.

## Success Criteria (Acceptance Criteria)

From task definition:

1. ✅ **ClaudeClient struct** - Session management
   - Maintains long-running session connection
   - Configuration for model, system prompt, permission mode
   - Thread-safe (Send + Sync)

2. ✅ **Message sending** - Interactive messaging
   - `send_message()` method that queues messages
   - Support for streaming responses via Control Protocol
   - Async API using tokio

3. ✅ **Streaming responses** - Receive agent outputs
   - Stream responses from the agent
   - Support for delta updates (text/tool_use streaming)
   - Handle end-of-stream signals

4. ✅ **Session control** - Interrupt and mode changes
   - `interrupt()` method to stop current streaming
   - Change permission_mode during session
   - Change model during session via control protocol

5. ✅ **Integration with Control Protocol** - Use existing infrastructure
   - Leverage rusty_claw-91n (ControlProtocolHandler)
   - Use rusty_claw-sna patterns (query() function)
   - Work with existing Transport and Message layers

6. ✅ **Comprehensive tests**
   - Unit tests for client operations
   - Integration tests with mock control protocol
   - Streaming response tests
   - ~20-30 tests, zero clippy warnings

7. ✅ **Complete documentation**
   - Module-level docs with usage examples
   - Examples showing session management and streaming
   - API documentation for all public methods

## Estimated Time

**Total:** ~9.5 hours (570 minutes)

**Breakdown by phase:**
- Phase 1: Basic Structure - 60 min
- Phase 2: Session Management - 90 min
- Phase 3: Message Sending - 90 min
- Phase 4: Streaming Response - 120 min
- Phase 5: Control Operations - 60 min
- Phase 6: Handler Registration - 30 min
- Phase 7: Integration Tests - 90 min
- Phase 8: Documentation - 60 min
- Phase 9: Verification - 30 min

**Notes:**
- This is comprehensive implementation with thorough testing
- Production-ready quality standards
- No cutting corners on tests or documentation

## Next Steps

1. ✅ Investigation complete
2. → Start Phase 1: Create basic structure
3. → Iterate through phases 2-9
4. → Final verification and close task

## References

**Python SDK:**
- https://github.com/anthropics/claude-agent-sdk-python

**Related tasks:**
- rusty_claw-91n: Control Protocol handler (COMPLETE)
- rusty_claw-sna: query() function (COMPLETE)

**Downstream blockers:**
- rusty_claw-isy: Add integration tests [P2]
- rusty_claw-b4s: Implement subagent support [P3]
- rusty_claw-bkm: Write examples [P3]

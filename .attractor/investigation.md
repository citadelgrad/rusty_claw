# Investigation: rusty_claw-6cn - Implement Transport trait and SubprocessCLITransport

**Task ID:** rusty_claw-6cn
**Priority:** P1 (Critical)
**Status:** IN_PROGRESS
**Date:** 2026-02-13

## Overview

This task implements the Transport abstraction layer that enables communication between the SDK and the Claude Code CLI. The Transport trait provides an async interface for subprocess management, stdin/stdout communication, and NDJSON message streaming.

## Current State

### Existing Code
- ✅ **Error hierarchy** - Fully implemented in `crates/rusty_claw/src/error.rs`
  - `ClawError::CliNotFound` - For CLI discovery failures
  - `ClawError::Connection` - For transport connection failures
  - `ClawError::Process` - For CLI crashes with exit codes and stderr
  - `ClawError::JsonDecode` - For NDJSON parsing failures (auto-converts from serde_json::Error)
  - `ClawError::Io` - For I/O operations (auto-converts from std::io::Error)

- ✅ **Message types** - Fully implemented in `crates/rusty_claw/src/messages.rs`
  - `Message` enum (System, Assistant, User, Result)
  - `ContentBlock` enum (Text, ToolUse, ToolResult, Thinking)
  - `SystemMessage`, `ResultMessage` - Tagged enums
  - Supporting structs: `ApiMessage`, `UsageInfo`, `ToolInfo`, `McpServerInfo`, `StreamEvent`

- ✅ **Dependencies available**
  - `tokio = { version = "1.35", features = ["full"] }` - Process management, async I/O, channels
  - `tokio-stream = "0.1"` - Stream utilities
  - `serde_json = "1"` - JSON parsing
  - `async-trait = "0.1"` - Async trait support for dyn dispatch
  - `tracing = "0.1"` - Logging

### Missing Implementation
- ❌ **Transport module** - Does not exist
  - `lib.rs:44-46` defines placeholder `pub mod transport` but no actual code
  - Need to create `crates/rusty_claw/src/transport/mod.rs`
  - Need to create `crates/rusty_claw/src/transport/subprocess.rs`

## Requirements Analysis

### 1. Transport Trait (SPEC.md:105-135)

**Location:** `crates/rusty_claw/src/transport/mod.rs`

```rust
#[async_trait]
pub trait Transport: Send + Sync {
    async fn connect(&mut self) -> Result<(), ClawError>;
    async fn write(&self, message: &[u8]) -> Result<(), ClawError>;
    fn messages(&self) -> mpsc::UnboundedReceiver<Result<serde_json::Value, ClawError>>;
    async fn end_input(&self) -> Result<(), ClawError>;
    async fn close(&mut self) -> Result<(), ClawError>;
    fn is_ready(&self) -> bool;
}
```

**Key characteristics:**
- Async methods for I/O operations
- `Send + Sync` bounds for thread safety
- Returns `UnboundedReceiver` for message streaming (not `impl Stream`)
- Uses `serde_json::Value` for raw message parsing (typed parsing happens at higher layers)
- `connect()` and `close()` take `&mut self` (state changes)
- `write()` and `is_ready()` take `&self` (concurrent access)

### 2. SubprocessCLITransport (SPEC.md:137-158)

**Location:** `crates/rusty_claw/src/transport/subprocess.rs`

**CLI Invocation:**
```bash
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

**Implementation requirements:**

1. **Process Management**
   - Use `tokio::process::Command` with piped stdin/stdout/stderr
   - Store `Child` handle for lifecycle management
   - Capture stderr for error diagnostics

2. **Message Streaming** (SPEC.md:159-170)
   - Spawn background `tokio::task` to read stdout line-by-line
   - Parse each line as `serde_json::Value`
   - Send parsed messages through `mpsc::unbounded_channel`
   - Send `ClawError::JsonDecode` for malformed JSON

3. **Stdin Writing**
   - Store `ChildStdin` handle (wrapped in `Arc<Mutex<>>` for concurrent access)
   - Write raw bytes to stdin
   - Handle write errors → `ClawError::Io`

4. **Process Lifecycle** (SPEC.md:817-821)
   - Graceful shutdown: close stdin, wait for exit
   - Forced shutdown: SIGTERM → SIGKILL after 5s timeout
   - Detect unexpected exits: background task monitors process status
   - Implement via `Drop` trait for cleanup

5. **State Tracking**
   - Track connection state (connected/disconnected)
   - `is_ready()` checks if process is alive and connected

### 3. NDJSON Framing (SPEC.md:159-170)

**Message format:**
```
{"type":"system","subtype":"init","session_id":"abc123",...}\n
{"type":"assistant","message":{"role":"assistant","content":[...]},...}\n
{"type":"result","subtype":"success","result":"...",...}\n
```

**Parsing strategy:**
- Read stdout line-by-line using `BufReader::lines()`
- Parse each line with `serde_json::from_str::<Value>(line)`
- Do NOT parse into typed `Message` here (that's protocol layer's job)
- Send `Result<Value, ClawError>` through channel

## Files to Create

### 1. `crates/rusty_claw/src/transport/mod.rs`
**Purpose:** Module root and Transport trait definition

**Contents:**
- Transport trait with all 6 methods
- Re-export SubprocessCLITransport
- Module documentation

**Size estimate:** ~80 lines (trait + docs)

### 2. `crates/rusty_claw/src/transport/subprocess.rs`
**Purpose:** SubprocessCLITransport implementation

**Contents:**
- `SubprocessCLITransport` struct with fields:
  - `child: Option<Child>` - Process handle
  - `stdin: Arc<Mutex<Option<ChildStdin>>>` - Stdin writer
  - `messages_rx: Option<UnboundedReceiver<...>>` - Message receiver
  - `connected: Arc<AtomicBool>` - Connection state
  - `cli_path: PathBuf` - Path to claude CLI
  - `args: Vec<String>` - CLI arguments

- Constructor: `pub fn new(cli_path: PathBuf, args: Vec<String>) -> Self`
- Transport trait implementation
- `Drop` implementation for cleanup
- Helper methods:
  - `spawn_reader_task()` - Background stdout reader
  - `graceful_shutdown()` - SIGTERM → wait → SIGKILL

**Size estimate:** ~300-400 lines (struct + impl + Drop + helpers + tests)

### 3. `crates/rusty_claw/src/lib.rs` (modify)
**Changes:**
- Remove placeholder `pub mod transport {}` (lines 44-46)
- Add: `pub mod transport;`
- Add Transport trait to prelude: `pub use crate::transport::Transport;`

## Implementation Strategy

### Phase 1: Transport Trait (30 min)
1. Create `crates/rusty_claw/src/transport/mod.rs`
2. Define Transport trait with all method signatures
3. Add comprehensive documentation
4. Export from lib.rs

### Phase 2: SubprocessCLITransport Structure (45 min)
1. Create `crates/rusty_claw/src/transport/subprocess.rs`
2. Define struct with all fields
3. Implement constructor
4. Implement `is_ready()`

### Phase 3: Process Spawning (45 min)
1. Implement `connect()` method:
   - Create `tokio::process::Command`
   - Set up stdin/stdout/stderr pipes
   - Spawn process
   - Store handles

### Phase 4: Message Streaming (60 min)
1. Implement `messages()` method:
   - Create `mpsc::unbounded_channel`
   - Spawn background task to read stdout
   - Parse NDJSON line-by-line
   - Send messages through channel
2. Handle parse errors gracefully

### Phase 5: Stdin Writing (30 min)
1. Implement `write()` method:
   - Lock stdin mutex
   - Write bytes
   - Handle errors

### Phase 6: Process Lifecycle (60 min)
1. Implement `end_input()` - Close stdin
2. Implement `close()` - Graceful shutdown
3. Implement `Drop` - Forced cleanup
4. Add process monitoring task

### Phase 7: Testing (60 min)
1. Unit tests for Transport trait interface
2. Integration tests with mock CLI
3. Process lifecycle tests
4. Error handling tests
5. NDJSON parsing tests

**Total estimate:** ~5-6 hours

## Dependencies

### Satisfied
- ✅ **rusty_claw-9pf** (Define error hierarchy) - CLOSED
  - All error variants available
  - Auto-conversion from `std::io::Error` and `serde_json::Error`

### Blocks
- ⚠️ **rusty_claw-91n** (Implement Control Protocol handler) - P1
  - Needs Transport trait for control protocol layer
- ⚠️ **rusty_claw-sna** (Implement query() function) - P1
  - Needs Transport for message streaming

## Risks & Challenges

### 1. Process Lifecycle Management ⚠️ MEDIUM RISK
**Issue:** Ensuring clean shutdown without orphaned processes or deadlocks

**Challenges:**
- Coordinating stdin close + stdout reader task + process wait
- Handling SIGTERM → SIGKILL escalation
- Detecting unexpected process exits
- Avoiding race conditions between `Drop` and explicit `close()`

**Mitigation:**
- Use `Arc<AtomicBool>` for connection state (lock-free)
- Background monitoring task detects unexpected exits
- `Drop` implementation uses non-blocking cleanup
- Timeout-based SIGKILL escalation (5s)

### 2. Message Channel Design ⚠️ MEDIUM RISK
**Issue:** `messages()` returns owned `UnboundedReceiver`, but Transport trait requires `&self`

**Challenges:**
- Can't move receiver out on each call
- Need to create receiver during `connect()`, return on first `messages()` call
- Subsequent calls should return error or panic

**Mitigation:**
- Store `Option<UnboundedReceiver>` in struct
- `messages()` uses `Option::take()` to move receiver out
- Document that `messages()` can only be called once
- Alternative: Return `mpsc::UnboundedSender` and clone receiver (more flexible)

### 3. Concurrent stdin access ⚠️ LOW RISK
**Issue:** Multiple tasks may call `write()` concurrently

**Mitigation:**
- Wrap `ChildStdin` in `Arc<Mutex<Option<ChildStdin>>>`
- Lock before each write
- Handle write errors properly

### 4. stderr capture ⚠️ LOW RISK
**Issue:** Need to capture stderr for error reporting but not block on it

**Mitigation:**
- Spawn separate task to read stderr
- Store in `Arc<Mutex<String>>` or channel
- Include in `ClawError::Process` on failure

## Testing Strategy

### Unit Tests
1. **Transport trait interface**
   - Verify all methods are callable
   - Check Send + Sync bounds

2. **SubprocessCLITransport construction**
   - Valid CLI path
   - Invalid CLI path → connection error

3. **NDJSON parsing**
   - Valid JSON lines → Ok(Value)
   - Invalid JSON → ClawError::JsonDecode
   - Empty lines → skip
   - Partial lines → buffer until complete

### Integration Tests
1. **Mock CLI process**
   - Use test binary that outputs known NDJSON
   - Verify messages are received
   - Verify stdin is written correctly

2. **Process lifecycle**
   - Normal shutdown (close stdin → process exits)
   - Forced shutdown (SIGTERM → SIGKILL)
   - Unexpected exit detection

3. **Error handling**
   - Process crashes → ClawError::Process with stderr
   - Write to closed stdin → ClawError::Io
   - Read from closed stdout → channel closes

### Property Tests (future)
- Concurrent writes don't corrupt stdin
- All spawned tasks are cleaned up
- No resource leaks

## Success Criteria

- ✅ Transport trait fully defined with all 6 methods
- ✅ SubprocessCLITransport compiles and implements Transport
- ✅ Process spawning works with correct CLI args
- ✅ NDJSON messages stream through channel
- ✅ stdin writes work correctly
- ✅ Graceful shutdown (close stdin → wait)
- ✅ Forced shutdown (SIGTERM → SIGKILL after timeout)
- ✅ Unexpected exit detection
- ✅ All unit tests passing
- ✅ Zero clippy warnings in new code
- ✅ Comprehensive documentation

## References

- **SPEC.md:103-135** - Transport trait specification
- **SPEC.md:137-158** - SubprocessCLITransport details
- **SPEC.md:159-170** - NDJSON message framing
- **SPEC.md:817-821** - Process lifecycle notes
- **error.rs** - ClawError variants
- **messages.rs** - Message types (for reference, not used here)
- **tokio docs** - process::Command, BufReader, channels

## Next Steps

1. Create transport module structure
2. Define Transport trait
3. Implement SubprocessCLITransport
4. Write comprehensive tests
5. Verify with `cargo check`, `cargo test`, `cargo clippy`
6. Update `.attractor/implementation-summary.md`
7. Close task with `bd close rusty_claw-6cn`

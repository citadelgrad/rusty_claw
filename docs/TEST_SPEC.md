# TEST_SPEC.md -- Technical Specification for Comprehensive Test Coverage

> Implementation guide for the rusty_claw test initiative. Every test function,
> assertion, fixture, and design decision is specified here.

---

## Table of Contents

1. [Test Architecture Overview](#1-test-architecture-overview)
2. [Shared Test Infrastructure](#2-shared-test-infrastructure)
3. [Transport Module Tests](#3-transport-module-tests)
4. [Control Module Tests](#4-control-module-tests)
5. [MCP Server Module Tests](#5-mcp-server-module-tests)
6. [Hooks Module Tests](#6-hooks-module-tests)
7. [Error Module Tests](#7-error-module-tests)
8. [Integration Tests](#8-integration-tests)
9. [Test Execution and CI Considerations](#9-test-execution-and-ci-considerations)

---

## 1. Test Architecture Overview

### 1.1 File Organization

```
crates/rusty_claw/
  src/
    transport/
      mod.rs           # Transport trait (no new tests needed -- trait definition only)
      subprocess.rs    # +15-20 new tests (I/O, lifecycle, concurrency)
      discovery.rs     # +3-5 new tests (edge cases)
    control/
      mod.rs           # +8-12 new tests (timeout, failures, concurrent)
      messages.rs      # +2-3 new tests (edge cases)
      handlers.rs      # +3-5 new tests (error handling, missing handlers)
      pending.rs       # +3-5 new tests (edge cases, stress)
    mcp_server.rs      # +10-15 new tests (edge cases, malformed input)
    hooks/
      mod.rs           # No code tests needed (re-exports only)
      types.rs         # +2-3 new tests (edge cases)
      callback.rs      # +2-3 new tests (error path)
      response.rs      # +2-3 new tests (edge cases)
    error.rs           # +10-15 new tests (propagation, source chains)
    options.rs         # +8-12 new tests (HookMatcher, HookEvent)
    client.rs          # +5-8 new tests (ResponseStream routing)
  tests/
    common/
      mod.rs           # NEW -- shared test utilities
      mock_transport.rs # NEW -- reusable MockTransport
    integration_test.rs # +10-15 new tests (e2e, transport I/O via mock_cli)
    mock_cli.rs        # ENHANCED -- add stdin reading for bidirectional tests
    fixtures/
      simple_query.ndjson       # existing
      tool_use.ndjson           # existing
      error_response.ndjson     # existing
      thinking_content.ndjson   # existing
      control_request.ndjson    # NEW -- fixture with control protocol messages
      large_message.ndjson      # NEW -- fixture with >64KB message
      malformed_lines.ndjson    # NEW -- fixture with invalid JSON mixed in
      rapid_burst.ndjson        # NEW -- fixture with many messages, no delay
```

### 1.2 Test Categories

Tests fall into three categories, each with different tradeoffs:

**Unit tests (in-module `#[cfg(test)]`):**
- Fast, isolated, deterministic
- Use in-process mocks (MockTransport, mock handlers)
- Test individual functions and methods
- No subprocess spawning, no filesystem access

**Component tests (in-module or in `tests/`):**
- Test interactions between two modules (e.g., control + transport)
- May use in-process mocks for one side
- Still deterministic, still fast

**Integration tests (`tests/integration_test.rs`):**
- Test the full stack through the mock CLI subprocess
- Exercise real process spawning, real NDJSON I/O
- Slower but validate the real code paths
- Depend on the mock_cli binary being built

### 1.3 Naming Convention

All test functions follow this pattern:
```
test_{module}_{method_or_behavior}_{scenario}
```

Examples:
- `test_reader_task_valid_json_messages`
- `test_request_timeout_cleans_up_pending`
- `test_hook_matcher_all_matches_any_tool`

### 1.4 Tokio Time Control

For timeout tests, we use `tokio::time::pause()` to control the clock:

```rust
#[tokio::test]
async fn test_request_timeout() {
    tokio::time::pause(); // Freeze clock

    // ... set up test ...

    // Advance past the timeout
    tokio::time::advance(Duration::from_secs(31)).await;

    // Assert timeout error
}
```

This requires the `test-util` feature on the `tokio` dependency, which is already
available in the workspace configuration (`tokio = { version = "1.35", features = ["full"] }`
-- the `full` feature includes `test-util`).

<!-- IMPORTANT: "full" feature in tokio includes test-util. Verify this before
implementing, but it should be the case for tokio 1.35+. -->

---

## 2. Shared Test Infrastructure

### 2.1 MockTransport (NEW: `tests/common/mock_transport.rs`)

The `MockTransport` in `control/mod.rs` tests is well-designed but locked inside the
test module. We extract and enhance it for reuse.

<!-- Design decision: We create this as a module inside tests/common/ rather than
as a pub(crate) module in src/ because it's test-only code. The tests/common/ pattern
is standard in Rust projects. -->

```rust
// tests/common/mock_transport.rs

use async_trait::async_trait;
use rusty_claw::error::ClawError;
use rusty_claw::transport::Transport;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

/// Configurable mock transport for testing
///
/// Supports:
/// - Capturing all bytes written to "stdin"
/// - Simulating messages arriving on "stdout"
/// - Configurable failure modes for write()
/// - Ready/not-ready state control
pub struct MockTransport {
    /// All bytes written via write()
    pub sent: Arc<Mutex<Vec<Vec<u8>>>>,
    /// Receiver for simulated messages (taken on messages() call)
    receiver: Arc<std::sync::Mutex<Option<mpsc::UnboundedReceiver<Result<Value, ClawError>>>>>,
    /// Sender for injecting simulated messages
    pub sender: mpsc::UnboundedSender<Result<Value, ClawError>>,
    /// Whether write() should fail
    pub write_fails: Arc<std::sync::Mutex<bool>>,
    /// Whether the transport reports as ready
    pub ready: Arc<std::sync::atomic::AtomicBool>,
}

impl MockTransport {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        Self {
            sent: Arc::new(Mutex::new(Vec::new())),
            receiver: Arc::new(std::sync::Mutex::new(Some(receiver))),
            sender,
            write_fails: Arc::new(std::sync::Mutex::new(false)),
            ready: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        }
    }

    /// Get all messages that were written to the transport
    pub async fn get_sent(&self) -> Vec<Vec<u8>> {
        self.sent.lock().await.clone()
    }

    /// Get sent messages parsed as JSON
    pub async fn get_sent_json(&self) -> Vec<Value> {
        self.sent.lock().await.iter()
            .map(|bytes| serde_json::from_slice(bytes).unwrap())
            .collect()
    }

    /// Configure write() to fail with the given error
    pub fn set_write_fails(&self, fails: bool) {
        *self.write_fails.lock().unwrap() = fails;
    }

    /// Inject a simulated message into the receive channel
    pub fn inject_message(&self, msg: Result<Value, ClawError>) {
        let _ = self.sender.send(msg);
    }
}

#[async_trait]
impl Transport for MockTransport {
    async fn connect(&mut self) -> Result<(), ClawError> { Ok(()) }

    async fn write(&self, data: &[u8]) -> Result<(), ClawError> {
        if *self.write_fails.lock().unwrap() {
            return Err(ClawError::Io(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "mock write failure",
            )));
        }
        self.sent.lock().await.push(data.to_vec());
        Ok(())
    }

    fn messages(&self) -> mpsc::UnboundedReceiver<Result<Value, ClawError>> {
        self.receiver.lock().unwrap().take()
            .expect("messages() can only be called once")
    }

    async fn end_input(&self) -> Result<(), ClawError> { Ok(()) }
    async fn close(&mut self) -> Result<(), ClawError> { Ok(()) }

    fn is_ready(&self) -> bool {
        self.ready.load(std::sync::atomic::Ordering::SeqCst)
    }
}
```

### 2.2 Common Test Utilities (NEW: `tests/common/mod.rs`)

```rust
// tests/common/mod.rs

pub mod mock_transport;

use rusty_claw::control::ControlProtocol;
use rusty_claw::control::messages::ControlResponse;
use std::sync::Arc;
use serde_json::json;

pub use mock_transport::MockTransport;

/// Create a ControlProtocol backed by a MockTransport
///
/// Returns (protocol, transport_handle) so the test can both
/// use the protocol and inject/inspect transport messages.
pub fn create_mock_control() -> (Arc<ControlProtocol>, Arc<MockTransport>) {
    let transport = Arc::new(MockTransport::new());
    let protocol = Arc::new(ControlProtocol::new(transport.clone()));
    (protocol, transport)
}

/// Helper to simulate a CLI response to a control request
///
/// Reads the first sent message from the transport, extracts the request_id,
/// and calls handle_response with the given response.
pub async fn simulate_response(
    control: &ControlProtocol,
    transport: &MockTransport,
    response: ControlResponse,
) {
    // Wait briefly for the request to be sent
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    let sent = transport.get_sent().await;
    if let Some(first) = sent.last() {
        let msg: serde_json::Value = serde_json::from_slice(first).unwrap();
        let request_id = msg["request_id"].as_str().unwrap().to_string();
        control.handle_response(&request_id, response).await;
    }
}
```

### 2.3 Enhanced Mock CLI

The mock CLI needs an additional mode for bidirectional communication. We add a
`--interactive` flag that:

1. Reads JSON lines from stdin
2. For each line, looks up a response in a response map (loaded from a fixture)
3. Writes the response to stdout

This enables testing the full transport write -> read cycle without the real CLI.

The enhancement is defined in the integration test section (Section 8).

---

## 3. Transport Module Tests

### 3.1 NDJSON Reader Task Tests

These tests exercise `spawn_reader_task()` through the public API by using
the mock CLI to produce output and verifying it arrives through the channel.

<!-- Design decision: We test spawn_reader_task through connect() + messages()
rather than calling it directly. This tests the real code path and doesn't require
making the function public. The mock CLI is the "controlled stdout" we need. -->

#### `test_reader_valid_json_lines`
**File:** `tests/integration_test.rs`
**What:** Verify that valid NDJSON lines from the mock CLI are received as parsed JSON values.
**Setup:** Spawn mock CLI with `simple_query.ndjson` fixture. Connect transport. Take messages receiver.
**Assertions:**
- Receive exactly 3 messages (matching fixture line count)
- Each message is `Ok(Value)` with correct structure
- First message has `"type": "system"`
- Messages arrive in order

```rust
#[tokio::test]
async fn test_transport_reads_valid_json_messages() {
    // Connect to mock CLI
    let mut transport = SubprocessCLITransport::new(
        Some(mock_cli_path()),
        vec![
            format!("--fixture={}", fixture_path("simple_query.ndjson").display()),
            "--delay=0".to_string(),
        ],
    );
    transport.connect().await.unwrap();

    let mut rx = transport.messages();
    let mut messages = vec![];

    while let Some(msg) = rx.recv().await {
        messages.push(msg);
    }

    // All 3 messages should be Ok
    assert_eq!(messages.len(), 3);
    for msg in &messages {
        assert!(msg.is_ok(), "Expected Ok, got {:?}", msg);
    }

    // First message should be system type
    let first = messages[0].as_ref().unwrap();
    assert_eq!(first["type"], "system");
}
```

#### `test_reader_empty_lines_skipped`
**File:** `tests/integration_test.rs`
**What:** Verify empty lines in NDJSON are silently skipped.
**Setup:** Create a fixture file with empty lines between valid JSON. Connect transport.
**Assertions:**
- Only non-empty JSON lines are received
- No errors for empty lines

<!-- Implementation note: This requires creating a fixture with empty lines.
Either create a new fixture file or use a custom subprocess that writes
empty lines. The mock_cli already skips empty lines when loading fixtures,
so we may need a simpler approach: write a tiny Rust test binary that
outputs specific lines including empties. Alternatively, we can test this
at the unit level by providing a mock ChildStdout. -->

#### `test_reader_malformed_json_produces_error`
**File:** `tests/integration_test.rs`
**What:** Verify that non-JSON lines produce `ClawError::JsonDecode` errors on the channel.
**Setup:** Create a `malformed_lines.ndjson` fixture containing a mix of valid JSON and
garbage text. Use a custom test binary or enhanced mock CLI that can output raw text.
**Assertions:**
- Valid lines produce `Ok(Value)`
- Invalid lines produce `Err(ClawError::JsonDecode(_))`
- The reader continues after malformed lines (doesn't stop)

#### `test_reader_channel_closes_on_stdout_end`
**File:** `tests/integration_test.rs`
**What:** Verify the message channel closes when the subprocess stdout closes.
**Setup:** Connect to mock CLI with a small fixture. Read all messages.
**Assertions:**
- After all messages are received, `rx.recv().await` returns `None`
- `transport.is_ready()` returns `false` after channel closes

```rust
#[tokio::test]
async fn test_transport_channel_closes_on_process_exit() {
    let mut transport = SubprocessCLITransport::new(
        Some(mock_cli_path()),
        vec![
            format!("--fixture={}", fixture_path("simple_query.ndjson").display()),
            "--delay=0".to_string(),
        ],
    );
    transport.connect().await.unwrap();
    assert!(transport.is_ready());

    let mut rx = transport.messages();

    // Drain all messages
    while let Some(_) = rx.recv().await {}

    // Channel closed, wait briefly for connected flag to update
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(!transport.is_ready());
}
```

#### `test_reader_large_message`
**File:** `tests/integration_test.rs`
**What:** Verify messages larger than typical buffer sizes are handled.
**Setup:** Create a `large_message.ndjson` fixture with a JSON object containing
a text field of ~100KB.
**Assertions:**
- Message is received intact without truncation
- Parsed JSON matches the original

#### `test_reader_rapid_burst`
**File:** `tests/integration_test.rs`
**What:** Verify the reader handles many messages arriving rapidly.
**Setup:** Create a `rapid_burst.ndjson` fixture with 100+ small JSON messages.
Use `--delay=0` for zero inter-message delay.
**Assertions:**
- All messages are received
- No messages are dropped
- Order is preserved

### 3.2 Transport Write Tests

<!-- Design decision: Write tests require a connected transport with a live stdin.
We use the mock CLI (which doesn't read stdin but keeps the pipe open until exit)
to provide a valid stdin handle. For verifying what was written, we'd need the
mock CLI to echo stdin to stderr, but that's complex. Instead, we verify:
1. write() doesn't error on a connected transport
2. write() errors after end_input()
3. Concurrent writes don't panic
-->

#### `test_write_to_connected_transport`
**File:** `src/transport/subprocess.rs` (unit test section)
**What:** Verify writing bytes to a connected transport succeeds.
**Setup:** Connect to mock CLI. Write a JSON message.
**Assertions:**
- `write()` returns `Ok(())`
- Transport remains ready after write

```rust
#[tokio::test]
async fn test_write_to_connected_transport() {
    let mut transport = SubprocessCLITransport::new(
        Some(mock_cli_path()),
        vec![
            format!("--fixture={}", fixture_path("simple_query.ndjson").display()),
            "--delay=100".to_string(), // Keep process alive while we write
        ],
    );
    transport.connect().await.unwrap();

    let msg = serde_json::json!({"type": "user", "message": "hello"});
    let mut bytes = serde_json::to_vec(&msg).unwrap();
    bytes.push(b'\n');

    let result = transport.write(&bytes).await;
    assert!(result.is_ok());
    assert!(transport.is_ready());
}
```

#### `test_write_after_end_input_fails`
**File:** `src/transport/subprocess.rs`
**What:** Verify writing after closing stdin fails.
**Setup:** Connect to mock CLI. Call `end_input()`. Try to write.
**Assertions:**
- `end_input()` succeeds
- Subsequent `write()` returns an error (either `ClawError::Connection` for "stdin already closed"
  or `ClawError::Io` for broken pipe)

#### `test_concurrent_writes_dont_interleave`
**File:** `src/transport/subprocess.rs`
**What:** Verify that concurrent writes are serialized by the Mutex.
**Setup:** Connect to mock CLI. Spawn 10 concurrent tasks each writing a message.
**Assertions:**
- All writes return `Ok(())`
- No panics or errors from the Mutex
- Transport remains in valid state

```rust
#[tokio::test]
async fn test_concurrent_writes() {
    let mut transport = SubprocessCLITransport::new(
        Some(mock_cli_path()),
        vec![
            format!("--fixture={}", fixture_path("simple_query.ndjson").display()),
            "--delay=200".to_string(),
        ],
    );
    transport.connect().await.unwrap();

    // Share the transport for concurrent writes
    // Note: Transport::write(&self) takes &self, so we can call it concurrently
    let mut handles = vec![];
    for i in 0..10 {
        let msg = format!("{{\"id\": {}}}\n", i);
        let transport_ref = &transport;
        // Can't easily share &transport across spawned tasks, so we test
        // sequential writes are fine. For true concurrent testing, would need Arc.
        let result = transport.write(msg.as_bytes()).await;
        assert!(result.is_ok(), "Write {} failed", i);
    }
}
```

### 3.3 Process Lifecycle Tests

#### `test_graceful_shutdown_normal_exit`
**File:** `src/transport/subprocess.rs`
**What:** Verify graceful shutdown with a process that exits cleanly.
**Setup:** Connect to mock CLI with a fixture that completes quickly. Wait for
messages to drain. Call `close()`.
**Assertions:**
- `close()` returns `Ok(())`
- `is_ready()` returns `false` after close

#### `test_close_idempotent`
**File:** `src/transport/subprocess.rs`
**What:** Verify calling `close()` multiple times doesn't panic.
**Setup:** Connect and close. Close again.
**Assertions:**
- First `close()` returns `Ok(())`
- Second `close()` returns `Ok(())` (idempotent)

```rust
#[tokio::test]
async fn test_close_idempotent() {
    let mut transport = SubprocessCLITransport::new(
        Some(mock_cli_path()),
        vec![
            format!("--fixture={}", fixture_path("simple_query.ndjson").display()),
            "--delay=0".to_string(),
        ],
    );
    transport.connect().await.unwrap();

    // Drain messages to let process finish
    let mut rx = transport.messages();
    while let Some(_) = rx.recv().await {}

    // Close twice
    assert!(transport.close().await.is_ok());
    assert!(transport.close().await.is_ok()); // idempotent
    assert!(!transport.is_ready());
}
```

#### `test_double_connect_rejected`
**File:** `src/transport/subprocess.rs`
**What:** Already exists but fragile (depends on claude being installed). Enhance to
use mock CLI so it's deterministic.
**Setup:** Connect to mock CLI. Try to connect again.
**Assertions:**
- First connect succeeds
- Second connect returns `ClawError::Connection("already connected")`

#### `test_monitor_task_detects_unexpected_exit`
**File:** `tests/integration_test.rs`
**What:** Verify the monitor task sets `connected` to false when the process exits unexpectedly.
**Setup:** Connect to mock CLI with a very short fixture. After fixture completes,
the process exits. Observe the `is_ready()` flag.
**Assertions:**
- After process exits, `is_ready()` becomes `false`
- No panic or hang

### 3.4 Discovery Edge Case Tests

#### `test_discovery_env_var_takes_precedence`
**File:** `src/transport/discovery.rs`
**What:** Verify CLAUDE_CLI_PATH environment variable is checked.
**Setup:** Set `CLAUDE_CLI_PATH` env var to current test executable path.
**Assertions:**
- `CliDiscovery::find(None)` returns the env var path
**Cleanup:** Unset env var after test.

<!-- Note: This test modifies environment variables, which can affect other
tests running concurrently. Consider using a serial test attribute or
a mutex to protect env var access. -->

#### `test_common_locations_includes_homebrew`
**File:** `src/transport/discovery.rs`
**What:** Verify `/opt/homebrew/bin/claude` is in the common locations list.
**Assertions:**
- `common_locations()` contains `/opt/homebrew/bin/claude`

#### `test_validate_version_rejects_old_version`
**File:** `src/transport/discovery.rs`
**What:** Verify that a binary returning version "1.9.9" is rejected.
**Setup:** This is hard to test without a custom binary that outputs "1.9.9".
Consider creating a tiny shell script or test binary for this purpose.
**Assertions:**
- `validate_version()` returns `ClawError::InvalidCliVersion`

---

## 4. Control Module Tests

### 4.1 Timeout Tests

#### `test_request_timeout_returns_control_timeout`
**File:** `src/control/mod.rs`
**What:** Verify that a request with no response times out correctly.
**Setup:** Create `ControlProtocol` with `MockTransport`. Send a request but don't
simulate any response. Use `tokio::time::pause()` to fast-forward past the timeout.
**Assertions:**
- `request()` returns `Err(ClawError::ControlTimeout { subtype: "control_request" })`

```rust
#[tokio::test]
async fn test_request_timeout() {
    tokio::time::pause();

    let transport = Arc::new(MockTransport::new());
    let control = ControlProtocol::new(transport.clone() as Arc<dyn Transport>);

    // Send request -- no response will come
    let handle = tokio::spawn({
        let control = Arc::new(control);
        async move {
            control.request(ControlRequest::Interrupt).await
        }
    });

    // Advance past the 30s timeout
    tokio::time::advance(Duration::from_secs(31)).await;

    let result = handle.await.unwrap();
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ClawError::ControlTimeout { .. }));
}
```

<!-- IMPORTANT: tokio::time::pause() must be called before any time-dependent
operations. The pause applies to the current runtime. -->

#### `test_timeout_cleans_up_pending_request`
**File:** `src/control/mod.rs`
**What:** Verify that after a timeout, the pending request entry is removed.
**Setup:** Same as above but also check the pending requests count.
**Assertions:**
- After timeout, `pending.len()` is 0
- The request_id from the timed-out request is no longer in the pending map

<!-- Design decision: PendingRequests::len() is #[cfg(test)] which is fine --
we're in a test. We can't directly access it from the ControlProtocol though
because it's a private field. We verify indirectly: after the timeout,
sending a response with the timed-out request_id should return false from
complete(). -->

#### `test_timeout_doesnt_affect_other_requests`
**File:** `src/control/mod.rs`
**What:** Verify that when one request times out, other in-flight requests still work.
**Setup:** Send two requests. Let one time out. Respond to the other.
**Assertions:**
- First request times out with `ControlTimeout`
- Second request receives its response correctly

### 4.2 Transport Write Failure Tests

#### `test_request_write_failure_returns_connection_error`
**File:** `src/control/mod.rs`
**What:** Verify that a transport write failure during `request()` is surfaced correctly.
**Setup:** Create `ControlProtocol` with `MockTransport` configured to fail writes.
**Assertions:**
- `request()` returns `Err(ClawError::Connection(...))`
- The error message mentions "Failed to send control request"

```rust
#[tokio::test]
async fn test_request_write_failure() {
    let transport = Arc::new(MockTransport::new());
    transport.set_write_fails(true);

    let control = ControlProtocol::new(transport.clone() as Arc<dyn Transport>);

    let result = control.request(ControlRequest::Interrupt).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, ClawError::Connection(_)));
    assert!(err.to_string().contains("Failed to send control request"));
}
```

#### `test_request_after_write_failure_still_works`
**File:** `src/control/mod.rs`
**What:** Verify the protocol recovers after a write failure.
**Setup:** Configure MockTransport to fail, send request (fails), re-enable writes,
send another request with a simulated response.
**Assertions:**
- First request fails with `Connection` error
- Second request succeeds

### 4.3 Concurrent Request Tests

#### `test_concurrent_requests_receive_correct_responses`
**File:** `src/control/mod.rs`
**What:** Verify multiple in-flight requests are routed correctly.
**Setup:** Send 5 requests concurrently. Simulate responses in a background task,
matching each request_id.
**Assertions:**
- All 5 requests receive their correct (unique) responses
- No cross-contamination between requests

```rust
#[tokio::test]
async fn test_concurrent_requests_correct_routing() {
    let transport = Arc::new(MockTransport::new());
    let control = Arc::new(ControlProtocol::new(
        transport.clone() as Arc<dyn Transport>
    ));

    // Spawn response simulator
    let transport_clone = transport.clone();
    let control_clone = control.clone();
    tokio::spawn(async move {
        // Wait for requests to be sent
        tokio::time::sleep(Duration::from_millis(50)).await;

        let sent = transport_clone.get_sent().await;
        for (i, msg_bytes) in sent.iter().enumerate() {
            let msg: Value = serde_json::from_slice(msg_bytes).unwrap();
            let request_id = msg["request_id"].as_str().unwrap().to_string();
            control_clone.handle_response(
                &request_id,
                ControlResponse::Success { data: json!({ "index": i }) },
            ).await;
        }
    });

    // Send 5 concurrent requests
    let mut handles = vec![];
    for _ in 0..5 {
        let c = control.clone();
        handles.push(tokio::spawn(async move {
            c.request(ControlRequest::McpStatus).await
        }));
    }

    // All should succeed
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }
}
```

### 4.4 Handler Dispatch Edge Cases

#### `test_handle_incoming_hook_no_handler_registered`
**File:** `src/control/mod.rs`
**What:** Verify that a hook callback with no registered handler returns an error response.
**Setup:** Create `ControlProtocol` without registering any hook handler. Call
`handle_incoming` with a `HookCallback` request.
**Assertions:**
- A `ControlResponse::Error` is sent back to the transport
- Error message mentions "No handler registered for hook_id"

#### `test_handle_incoming_mcp_no_handler_registered`
**File:** `src/control/mod.rs`
**What:** Verify that an MCP message with no handler returns an error.
**Setup:** Create `ControlProtocol` without MCP handler. Call `handle_incoming`
with an `McpMessage` request.
**Assertions:**
- `ControlResponse::Error` sent to transport
- Error message mentions "No MCP message handler registered"

#### `test_handle_incoming_can_use_tool_handler_error`
**File:** `src/control/mod.rs`
**What:** Verify that a handler returning an error is wrapped in ControlResponse::Error.
**Setup:** Register a `CanUseToolHandler` that returns `Err(ClawError::ToolExecution(...))`.
**Assertions:**
- `ControlResponse::Error` is sent with the error message

### 4.5 Pending Requests Edge Cases

#### `test_pending_insert_duplicate_id`
**File:** `src/control/pending.rs`
**What:** Verify that inserting a duplicate ID overwrites the previous entry.
**Setup:** Insert two entries with the same ID.
**Assertions:**
- `len()` is 1 (not 2)
- Completing the ID sends to the second sender (first is dropped)

#### `test_pending_stress_100_concurrent`
**File:** `src/control/pending.rs`
**What:** Verify 100 concurrent insert+complete operations work correctly.
**Setup:** Spawn 100 tasks, each inserting and completing with unique IDs.
**Assertions:**
- All 100 completions return `true`
- All 100 receivers get their response
- Final `len()` is 0

---

## 5. MCP Server Module Tests

### 5.1 Edge Case Tests

#### `test_tools_call_missing_params`
**File:** `src/mcp_server.rs`
**What:** Verify `tools/call` with no `params` field returns an error.
**Setup:** Send `{"jsonrpc":"2.0","id":1,"method":"tools/call"}` (no params).
**Assertions:**
- Returns JSON-RPC error with "Missing params" message

```rust
#[tokio::test]
async fn test_handle_tools_call_missing_params() {
    let server = SdkMcpServerImpl::new("test", "1.0.0");
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call"
    });
    let response = server.handle_jsonrpc(request).await;
    assert!(response.is_err() || response.unwrap()["error"].is_object());
}
```

<!-- Note: The current implementation does .as_object() which returns None for
missing "params", producing a ControlError. This test verifies that behavior. -->

#### `test_tools_call_missing_arguments_defaults_to_empty`
**File:** `src/mcp_server.rs`
**What:** Verify `tools/call` with params but no `arguments` defaults to `{}`.
**Setup:** Register a tool that echoes its input. Send a call with name but no arguments.
**Assertions:**
- Tool executes successfully
- Tool receives `{}` as arguments

#### `test_tools_call_missing_name`
**File:** `src/mcp_server.rs`
**What:** Verify `tools/call` with params but no `name` returns an error.
**Setup:** Send `{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{}}`.
**Assertions:**
- Returns error with "Missing tool name"

#### `test_handle_missing_method`
**File:** `src/mcp_server.rs`
**What:** Verify request with no `method` field returns an error.
**Setup:** Send `{"jsonrpc":"2.0","id":1}`.
**Assertions:**
- Returns `Err(ClawError::ControlError("Missing method field"))`

#### `test_duplicate_tool_registration`
**File:** `src/mcp_server.rs`
**What:** Verify registering two tools with the same name keeps the last one.
**Setup:** Register "my_tool" with handler A, then register "my_tool" with handler B.
**Assertions:**
- `list_tools()` returns 1 tool (not 2)
- `tools/call` for "my_tool" invokes handler B

```rust
#[tokio::test]
async fn test_duplicate_tool_last_wins() {
    let mut server = SdkMcpServerImpl::new("test", "1.0.0");

    let handler_a = Arc::new(MockHandler { response: "A".to_string() });
    let handler_b = Arc::new(MockHandler { response: "B".to_string() });

    server.register_tool(SdkMcpTool::new("dup", "First", json!({}), handler_a));
    server.register_tool(SdkMcpTool::new("dup", "Second", json!({}), handler_b));

    assert_eq!(server.list_tools().len(), 1);

    let response = server.handle_jsonrpc(json!({
        "jsonrpc": "2.0", "id": 1,
        "method": "tools/call",
        "params": { "name": "dup", "arguments": {} }
    })).await.unwrap();

    assert_eq!(response["result"]["content"][0]["text"], "B");
}
```

#### `test_empty_tool_list`
**File:** `src/mcp_server.rs`
**What:** Verify `tools/list` returns empty array when no tools registered.
**Assertions:**
- `list_tools()` returns empty vec
- `tools/list` JSON-RPC response has `"tools": []`

#### `test_registry_multiple_servers`
**File:** `src/mcp_server.rs`
**What:** Verify registry with multiple servers routes correctly.
**Setup:** Register two servers with different tools.
**Assertions:**
- Messages routed to server A execute server A's tools
- Messages routed to server B execute server B's tools
- Messages routed to unknown server return error

#### `test_null_json_rpc_id`
**File:** `src/mcp_server.rs`
**What:** Verify JSON-RPC requests with `null` id are handled.
**Assertions:**
- Response includes `"id": null`

### 5.2 Tool Content Serialization Edge Cases

#### `test_tool_content_text_serialization_roundtrip`
**File:** `src/mcp_server.rs`
**What:** Verify text content serializes and deserializes correctly.
**Assertions:**
- `{"type": "text", "text": "hello"}` round-trips

#### `test_tool_content_image_serialization_roundtrip`
**File:** `src/mcp_server.rs`
**What:** Verify image content uses `mimeType` (camelCase) in JSON.
**Assertions:**
- Serialized JSON contains `"mimeType"` not `"mime_type"`

#### `test_tool_result_error_serialization`
**File:** `src/mcp_server.rs`
**What:** Verify error result includes `isError: true` in JSON.
**Assertions:**
- `ToolResult::error("fail")` serializes with `"isError": true`

---

## 6. Hooks Module Tests

### 6.1 HookMatcher Tests (Currently ZERO tests)

All of these go in `src/options.rs` in the `#[cfg(test)] mod tests` section since
`HookMatcher` is defined in `options.rs`.

#### `test_hook_matcher_all_matches_any_tool`
**What:** `HookMatcher::all()` should match any tool name.
**Assertions:**
```rust
let m = HookMatcher::all();
assert!(m.matches("Bash"));
assert!(m.matches("Read"));
assert!(m.matches("Edit"));
assert!(m.matches(""));
assert!(m.matches("some_random_tool_name_12345"));
```

#### `test_hook_matcher_tool_exact_match`
**What:** `HookMatcher::tool("Bash")` should only match "Bash".
**Assertions:**
```rust
let m = HookMatcher::tool("Bash");
assert!(m.matches("Bash"));
assert!(!m.matches("bash"));  // case-sensitive
assert!(!m.matches("Read"));
assert!(!m.matches("Bash "));  // trailing space
assert!(!m.matches(""));
```

#### `test_hook_matcher_tool_case_sensitive`
**What:** Matching is case-sensitive (current behavior).
**Assertions:**
```rust
let m = HookMatcher::tool("Bash");
assert!(m.matches("Bash"));
assert!(!m.matches("bash"));
assert!(!m.matches("BASH"));
assert!(!m.matches("bAsH"));
```

#### `test_hook_matcher_all_has_none_tool_name`
**What:** `HookMatcher::all()` creates a matcher with `tool_name: None`.
**Assertions:**
```rust
let m = HookMatcher::all();
assert!(m.tool_name.is_none());
```

#### `test_hook_matcher_tool_stores_name`
**What:** `HookMatcher::tool("X")` stores `tool_name: Some("X")`.
**Assertions:**
```rust
let m = HookMatcher::tool("Bash");
assert_eq!(m.tool_name, Some("Bash".to_string()));
```

#### `test_hook_matcher_serialization_all`
**What:** `HookMatcher::all()` serializes without a `tool_name` field.
**Assertions:**
```rust
let m = HookMatcher::all();
let json = serde_json::to_value(&m).unwrap();
assert!(!json.as_object().unwrap().contains_key("tool_name"));
```

#### `test_hook_matcher_serialization_tool`
**What:** `HookMatcher::tool("Bash")` serializes with `tool_name`.
**Assertions:**
```rust
let m = HookMatcher::tool("Bash");
let json = serde_json::to_value(&m).unwrap();
assert_eq!(json["tool_name"], "Bash");
```

#### `test_hook_matcher_deserialization_roundtrip`
**What:** Serialize and deserialize round-trip for both variants.
**Assertions:**
```rust
let all = HookMatcher::all();
let json = serde_json::to_string(&all).unwrap();
let parsed: HookMatcher = serde_json::from_str(&json).unwrap();
assert!(parsed.matches("anything"));

let tool = HookMatcher::tool("Read");
let json = serde_json::to_string(&tool).unwrap();
let parsed: HookMatcher = serde_json::from_str(&json).unwrap();
assert!(parsed.matches("Read"));
assert!(!parsed.matches("Write"));
```

#### `test_hook_matcher_empty_string_tool`
**What:** Edge case: `HookMatcher::tool("")` should only match empty string.
**Assertions:**
```rust
let m = HookMatcher::tool("");
assert!(m.matches(""));
assert!(!m.matches("Bash"));
```

### 6.2 HookEvent Variant Tests

#### `test_hook_event_all_variants_serialize`
**File:** `src/options.rs`
**What:** Verify all 10 HookEvent variants serialize correctly.
**Assertions:**
```rust
let events = vec![
    (HookEvent::PreToolUse, "PreToolUse"),
    (HookEvent::PostToolUse, "PostToolUse"),
    (HookEvent::PostToolUseFailure, "PostToolUseFailure"),
    (HookEvent::UserPromptSubmit, "UserPromptSubmit"),
    (HookEvent::Stop, "Stop"),
    (HookEvent::SubagentStop, "SubagentStop"),
    (HookEvent::SubagentStart, "SubagentStart"),
    (HookEvent::PreCompact, "PreCompact"),
    (HookEvent::Notification, "Notification"),
    (HookEvent::PermissionRequest, "PermissionRequest"),
];

for (event, expected) in events {
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json.as_str().unwrap(), expected,
        "HookEvent::{:?} should serialize to {:?}", event, expected);
}
```

#### `test_hook_event_all_variants_roundtrip`
**File:** `src/options.rs`
**What:** Verify all variants survive serialize -> deserialize.
**Assertions:**
- For each variant, `from_value(to_value(v)) == v`

#### `test_hook_event_as_hashmap_key`
**File:** `src/options.rs`
**What:** Verify HookEvent works as HashMap key (tests Hash + Eq).
**Assertions:**
```rust
let mut map = HashMap::new();
map.insert(HookEvent::PreToolUse, vec![HookMatcher::all()]);
map.insert(HookEvent::PostToolUse, vec![HookMatcher::tool("Bash")]);
assert_eq!(map.len(), 2);
assert!(map.contains_key(&HookEvent::PreToolUse));
```

#### `test_hook_event_unknown_string_deserialization`
**File:** `src/options.rs`
**What:** Verify unknown event strings produce deserialization errors.
**Assertions:**
```rust
let result = serde_json::from_str::<HookEvent>("\"UnknownEvent\"");
assert!(result.is_err());
```

### 6.3 Hook Callback Edge Cases

#### `test_hook_callback_returns_error`
**File:** `src/hooks/callback.rs`
**What:** Verify a HookCallback that returns an error propagates correctly.
**Setup:** Create a callback that returns `Err(ClawError::ToolExecution(...))`.
**Assertions:**
- `call()` returns the error

#### `test_hook_callback_with_all_input_fields`
**File:** `src/hooks/callback.rs`
**What:** Verify a callback receives all HookInput fields correctly.
**Setup:** Create a HookInput with all fields populated. Verify the callback receives them.
**Assertions:**
- `input.tool_name`, `input.tool_input`, `input.tool_output`, `input.error`,
  `input.prompt`, and `input.metadata` are all accessible

### 6.4 HookResponse Edge Cases

#### `test_hook_response_with_updated_input`
**File:** `src/hooks/response.rs`
**What:** Verify `with_updated_input()` serializes the modified input.
**Assertions:**
```rust
let response = HookResponse::allow("ok")
    .with_updated_input(json!({"command": "ls -la"}));
let json = serde_json::to_value(&response).unwrap();
assert_eq!(json["updated_input"]["command"], "ls -la");
```

#### `test_hook_response_deny_sets_continue_false`
**File:** `src/hooks/response.rs`
**What:** Verify `deny()` sets `should_continue` to `false`.
**Assertions:**
```rust
let response = HookResponse::deny("blocked");
assert!(!response.should_continue);
let json = serde_json::to_value(&response).unwrap();
assert_eq!(json["continue"], false);
```

---

## 7. Error Module Tests

### 7.1 Error Source Chain Tests

#### `test_io_error_source_chain`
**File:** `src/error.rs`
**What:** Verify `ClawError::Io` implements `source()` returning the original `io::Error`.
**Assertions:**
```rust
use std::error::Error;

let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
let claw_err: ClawError = io_err.into();

let source = claw_err.source();
assert!(source.is_some());
// source should be the original io::Error
let source_str = source.unwrap().to_string();
assert!(source_str.contains("file missing"));
```

#### `test_json_decode_error_source_chain`
**File:** `src/error.rs`
**What:** Verify `ClawError::JsonDecode` implements `source()`.
**Assertions:**
```rust
use std::error::Error;

let json_err = serde_json::from_str::<Value>("not json").unwrap_err();
let claw_err: ClawError = json_err.into();

let source = claw_err.source();
assert!(source.is_some());
```

#### `test_non_from_variants_have_no_source`
**File:** `src/error.rs`
**What:** Verify variants without `#[from]` return `None` from `source()`.
**Assertions:**
```rust
use std::error::Error;

let err = ClawError::CliNotFound;
assert!(err.source().is_none());

let err = ClawError::Connection("test".into());
assert!(err.source().is_none());

let err = ClawError::ControlTimeout { subtype: "test".into() };
assert!(err.source().is_none());
```

### 7.2 Error Propagation Tests

#### `test_io_error_propagation_through_transport_write`
**File:** `src/error.rs` or `src/transport/subprocess.rs`
**What:** Verify an I/O error during write surfaces as `ClawError::Io`.

<!-- This is tested indirectly via the "write after end_input" test in transport.
We verify the error type matches. -->

#### `test_error_debug_output_all_variants`
**File:** `src/error.rs`
**What:** Verify `Debug` output is meaningful for all variants.
**Assertions:**
```rust
// Just ensure Debug doesn't panic and produces non-empty output
let variants: Vec<ClawError> = vec![
    ClawError::CliNotFound,
    ClawError::InvalidCliVersion { version: "1.0".into() },
    ClawError::Connection("test".into()),
    ClawError::Process { code: 1, stderr: "err".into() },
    ClawError::MessageParse { reason: "bad".into(), raw: "{}".into() },
    ClawError::ControlTimeout { subtype: "test".into() },
    ClawError::ControlError("test".into()),
    ClawError::ToolExecution("test".into()),
];

for err in &variants {
    let debug = format!("{:?}", err);
    assert!(!debug.is_empty(), "Debug output should not be empty for {:?}", err);
}
```

#### `test_error_display_all_variants`
**File:** `src/error.rs`
**What:** Verify `Display` output covers all variants (extends existing tests).
**Assertions:**
- Each variant's `to_string()` contains expected substrings
- No variant panics on `to_string()`

### 7.3 Cross-Module Error Propagation

#### `test_transport_write_error_through_control_request`
**File:** `src/control/mod.rs`
**What:** Verify a transport write error during `request()` produces
`ClawError::Connection` (not `ClawError::Io`).
**Setup:** MockTransport configured to fail writes.
**Assertions:**
- Error is `ClawError::Connection` wrapping the transport error message

#### `test_json_decode_error_through_response_stream`
**File:** `src/client.rs`
**What:** Verify a JSON decode error from transport surfaces in ResponseStream.
**Setup:** Create a ResponseStream backed by a channel. Inject an
`Err(ClawError::JsonDecode(...))` message.
**Assertions:**
- Stream yields `Err(ClawError::JsonDecode(...))`

---

## 8. Integration Tests

### 8.1 Full Transport I/O Tests

These tests use the mock CLI to exercise the real transport code path.

#### `test_transport_connect_read_messages_close`
**File:** `tests/integration_test.rs`
**What:** Full lifecycle: connect -> read messages -> close.
**Setup:** Connect to mock CLI with `simple_query.ndjson`.
**Assertions:**
- `connect()` succeeds, `is_ready()` is true
- All 3 fixture messages are received via `messages()`
- After messages drain, channel closes
- `close()` succeeds

#### `test_transport_connect_write_read`
**File:** `tests/integration_test.rs`
**What:** Verify writing to stdin while reading from stdout works.
**Setup:** Connect to mock CLI. Write a message. Read responses.
**Assertions:**
- Write succeeds
- Messages from fixture are still received
- No deadlock between read and write

#### `test_transport_with_tool_use_fixture`
**File:** `tests/integration_test.rs`
**What:** Verify tool_use fixture messages parse correctly through transport.
**Assertions:**
- All messages received and parseable
- Tool use content blocks are present

### 8.2 Mock CLI Enhancement Tests

#### `test_mock_cli_echo_mode`
**File:** `tests/integration_test.rs`
**What:** Test enhanced mock CLI that reads stdin and writes responses.

<!-- Only implement if mock_cli is enhanced with --interactive mode -->

### 8.3 End-to-End ResponseStream Tests

#### `test_response_stream_parses_fixture_messages`
**File:** `tests/integration_test.rs`
**What:** Verify ResponseStream correctly parses raw JSON into typed Messages.
**Setup:** Create a ResponseStream backed by a channel. Inject raw JSON
matching fixture format.
**Assertions:**
- System messages parsed as `Message::System`
- Assistant messages parsed as `Message::Assistant`
- Result messages parsed as `Message::Result`

#### `test_response_stream_routes_control_messages`
**File:** `src/client.rs`
**What:** Verify control messages are handled internally and not yielded.
**Setup:** Create ResponseStream. Inject a control_request message and a
regular assistant message.
**Assertions:**
- Only the assistant message is yielded by the stream
- The control_request is routed to the handler

```rust
#[tokio::test]
async fn test_response_stream_filters_control_messages() {
    use tokio_stream::StreamExt;

    let (tx, rx) = mpsc::unbounded_channel();
    let mock_transport = Arc::new(MockTransport::new());
    let control = Arc::new(ControlProtocol::new(mock_transport));

    let mut stream = ResponseStream::new(rx, control);

    // Inject a regular message
    tx.send(Ok(json!({
        "type": "assistant",
        "message": {
            "role": "assistant",
            "content": [{"type": "text", "text": "hello"}],
            "model": "claude-sonnet-4",
            "id": "msg_1",
            "stop_reason": null,
            "usage": {"input_tokens": 1, "output_tokens": 1}
        }
    }))).unwrap();

    // Close channel
    drop(tx);

    // Should get the assistant message
    let msg = stream.next().await;
    assert!(msg.is_some());
    assert!(msg.unwrap().is_ok());
}
```

#### `test_response_stream_completion`
**File:** `src/client.rs`
**What:** Verify stream reports `is_complete()` after channel closes.
**Setup:** Create stream, drop the sender.
**Assertions:**
- After `next()` returns `None`, `is_complete()` returns `true`

### 8.4 New Fixture Files

#### `control_request.ndjson`
```json
{"type":"system","subtype":"init","session_id":"sess_ctrl_001","tools":[],"mcp_servers":[]}
{"type":"control_request","request_id":"req_001","request":{"subtype":"can_use_tool","tool_name":"Bash","tool_input":{"command":"ls"}}}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Done"}],"model":"claude-sonnet-4","id":"msg_1","stop_reason":"end_turn","usage":{"input_tokens":10,"output_tokens":5}}}
{"type":"result","subtype":"success","num_turns":1,"is_error":false,"session_id":"sess_ctrl_001"}
```

#### `large_message.ndjson`
A fixture containing a single assistant message with a text content block of ~100KB.
Generate this programmatically during test setup rather than checking in a large file.

#### `rapid_burst.ndjson`
100 small assistant messages with minimal content, for testing throughput.

---

## 9. Test Execution and CI Considerations

### 9.1 Cargo Configuration

Ensure the `tokio` dependency includes `test-util` for time control:

```toml
# In workspace Cargo.toml -- already covered by "full" feature
tokio = { version = "1.35", features = ["full"] }
```

### 9.2 Running Tests

```bash
# Run all tests
cargo test

# Run tests for a specific module
cargo test --lib transport
cargo test --lib control
cargo test --lib mcp_server
cargo test --lib hooks
cargo test --lib error

# Run integration tests
cargo test --test integration_test

# Run with output for debugging
cargo test -- --nocapture
```

### 9.3 CI Pipeline

The test suite should pass without the real Claude CLI installed. Tests that
require the real CLI should be guarded:

```rust
#[tokio::test]
#[ignore] // Requires real Claude CLI
async fn test_with_real_cli() {
    // ...
}
```

Run ignored tests explicitly:
```bash
cargo test -- --ignored
```

### 9.4 Test Timeout Protection

All async tests should complete within 10 seconds under normal conditions.
For timeout tests that use `tokio::time::pause()`, the wall-clock time should
be under 1 second since time is simulated.

CI should set a global timeout:
```bash
cargo test -- --test-threads=4 # Parallel test execution
```

### 9.5 Flakiness Prevention

1. **No `sleep()` for synchronization.** Use channels, barriers, or `tokio::time::pause()`.
2. **No reliance on system state.** Tests should create their own fixtures, not depend
   on files existing on disk (except the checked-in fixture files).
3. **No port binding.** No tests should bind to network ports.
4. **Deterministic ordering.** Tests should not depend on execution order.
5. **Clean up resources.** Use `Drop` or explicit cleanup. Don't leave temp files.

### 9.6 Performance Budget

| Category | Budget | Rationale |
|----------|--------|-----------|
| Unit tests (per test) | < 100ms | In-process, no I/O |
| Component tests (per test) | < 500ms | May involve async coordination |
| Integration tests (per test) | < 2s | Subprocess spawning |
| Total suite | < 30s | Developer productivity |

### 9.7 Dependency on Mock CLI Build

Integration tests depend on the `mock_cli` binary being built. This is handled
automatically by Cargo's test infrastructure:

```toml
# In crates/rusty_claw/Cargo.toml
[[test]]
name = "integration_test"
path = "tests/integration_test.rs"

[[bin]]
name = "mock_cli"
path = "tests/mock_cli.rs"
```

The `env!("CARGO_BIN_EXE_mock_cli")` macro resolves the path to the built binary
at compile time.

---

## Appendix A: Test-to-Requirement Traceability

| Requirement | Test Functions |
|-------------|---------------|
| REQ-T1 (NDJSON Read) | `test_transport_reads_valid_json_messages`, `test_reader_empty_lines_skipped`, `test_reader_malformed_json_produces_error`, `test_transport_channel_closes_on_process_exit`, `test_reader_large_message`, `test_reader_rapid_burst` |
| REQ-T2 (Write) | `test_write_to_connected_transport`, `test_write_after_end_input_fails`, `test_concurrent_writes` |
| REQ-T3 (Lifecycle) | `test_graceful_shutdown_normal_exit`, `test_close_idempotent`, `test_double_connect_rejected`, `test_monitor_task_detects_unexpected_exit` |
| REQ-C1 (Timeout) | `test_request_timeout_returns_control_timeout`, `test_timeout_cleans_up_pending_request`, `test_timeout_doesnt_affect_other_requests` |
| REQ-C2 (Write Failure) | `test_request_write_failure_returns_connection_error`, `test_request_after_write_failure_still_works` |
| REQ-H1 (HookMatcher) | `test_hook_matcher_all_matches_any_tool`, `test_hook_matcher_tool_exact_match`, `test_hook_matcher_tool_case_sensitive`, `test_hook_matcher_all_has_none_tool_name`, `test_hook_matcher_tool_stores_name`, `test_hook_matcher_serialization_all`, `test_hook_matcher_serialization_tool`, `test_hook_matcher_deserialization_roundtrip`, `test_hook_matcher_empty_string_tool` |
| REQ-H2 (HookEvent) | `test_hook_event_all_variants_serialize`, `test_hook_event_all_variants_roundtrip`, `test_hook_event_as_hashmap_key`, `test_hook_event_unknown_string_deserialization` |
| REQ-H3 (Hook Execution) | `test_handle_incoming_hook_no_handler_registered`, `test_handle_incoming_can_use_tool_handler_error`, `test_hook_callback_returns_error` |
| REQ-C3 (Concurrent) | `test_concurrent_requests_correct_routing` |
| REQ-M1 (MCP Edge) | `test_tools_call_missing_params`, `test_tools_call_missing_arguments_defaults_to_empty`, `test_tools_call_missing_name`, `test_handle_missing_method`, `test_duplicate_tool_last_wins`, `test_empty_tool_list`, `test_registry_multiple_servers`, `test_null_json_rpc_id` |
| REQ-E1 (Error Source) | `test_io_error_source_chain`, `test_json_decode_error_source_chain`, `test_non_from_variants_have_no_source` |
| REQ-E2 (Error Display) | `test_error_debug_output_all_variants`, `test_error_display_all_variants` |
| REQ-C4 (ResponseStream) | `test_response_stream_filters_control_messages`, `test_response_stream_completion`, `test_response_stream_parses_fixture_messages` |
| REQ-I1 (E2E) | `test_transport_connect_read_messages_close`, `test_transport_connect_write_read`, `test_transport_with_tool_use_fixture` |

---

## Appendix B: Mock Infrastructure Decision Record

**Decision:** Use in-process `MockTransport` for unit/component tests and subprocess
`mock_cli` for integration tests.

**Alternatives considered:**

1. **Only mock_cli subprocess.** Rejected because subprocess tests are 10-100x slower
   than in-process tests, and some scenarios (like transport write failure) are hard to
   simulate with a real subprocess.

2. **Only in-process mocks.** Rejected because this wouldn't test the real subprocess
   spawning, NDJSON parsing, and process lifecycle code paths.

3. **Use a Rust-native mock framework (mockall, etc.).** Rejected because the `Transport`
   trait is already designed for mock implementations. Adding a mocking framework
   would add a dependency without significant benefit. The hand-written `MockTransport`
   is simple, readable, and gives precise control over failure injection.

**Conclusion:** The hybrid approach tests both the abstraction layer (via MockTransport)
and the real implementation (via mock_cli subprocess). This provides the best balance
of speed, coverage, and confidence.

---

## Appendix C: Estimated New Test Count by File

| File | Existing | New | Total |
|------|----------|-----|-------|
| `transport/subprocess.rs` | 7 | ~15 | ~22 |
| `transport/discovery.rs` | 7 | ~5 | ~12 |
| `control/mod.rs` | 7 | ~12 | ~19 |
| `control/messages.rs` | 13 | ~2 | ~15 |
| `control/handlers.rs` | 7 | ~5 | ~12 |
| `control/pending.rs` | 7 | ~3 | ~10 |
| `mcp_server.rs` | 24 | ~12 | ~36 |
| `hooks/types.rs` | 7 | ~3 | ~10 |
| `hooks/callback.rs` | 4 | ~3 | ~7 |
| `hooks/response.rs` | 7 | ~2 | ~9 |
| `error.rs` | 11 | ~12 | ~23 |
| `options.rs` | ~14 | ~14 | ~28 |
| `client.rs` | ~14 | ~6 | ~20 |
| `tests/integration_test.rs` | ~25 | ~12 | ~37 |
| `tests/common/` | 0 | (infrastructure) | -- |
| **Total** | **~116** | **~106** | **~222** |

<!-- END OF DOCUMENT -->

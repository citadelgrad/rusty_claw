set -e

# Epic: Comprehensive Test Coverage for rusty_claw
EPIC_ID=$(bd create --title='Comprehensive Test Coverage for rusty_claw' --type=epic --priority=2 \
  --description='Add ~106 new tests across all modules to achieve comprehensive test coverage. Covers transport, control, MCP server, hooks, error, and integration tests. Target: ~222 total tests from ~116 existing. See docs/TEST_SPEC.md for full specification.' \
  --notes='Spec: docs/TEST_SPEC.md. Estimated new tests by module: transport/subprocess ~15, transport/discovery ~5, control/mod ~12, control/pending ~3, mcp_server ~12, hooks/options ~14, error ~12, client ~6, integration ~12. Total suite budget: under 30s.' \
  --silent)

# ─────────────────────────────────────────────────────────────────────
# Task 1: Shared Test Infrastructure (MockTransport + Common Utilities)
# ─────────────────────────────────────────────────────────────────────

cat <<'ENDDESIGN' > /tmp/bd_design_1.txt
FILE: tests/common/mock_transport.rs (NEW)

Create a reusable MockTransport that implements the Transport trait. Extract and enhance
the existing MockTransport from control/mod.rs tests. Place in tests/common/ because
this is test-only code (standard Rust pattern).

Full implementation:

use async_trait::async_trait;
use rusty_claw::error::ClawError;
use rusty_claw::transport::Transport;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

pub struct MockTransport {
    pub sent: Arc<Mutex<Vec<Vec<u8>>>>,
    receiver: Arc<std::sync::Mutex<Option<mpsc::UnboundedReceiver<Result<Value, ClawError>>>>>,
    pub sender: mpsc::UnboundedSender<Result<Value, ClawError>>,
    pub write_fails: Arc<std::sync::Mutex<bool>>,
    pub ready: Arc<std::sync::atomic::AtomicBool>,
}

Methods needed:
- new() -> Self: Creates channels, initializes all fields
- get_sent() -> Vec<Vec<u8>>: Returns clone of all sent bytes
- get_sent_json() -> Vec<Value>: Returns sent bytes parsed as JSON
- set_write_fails(bool): Toggle write failure mode
- inject_message(Result<Value, ClawError>): Push message into receive channel

Transport impl:
- connect: no-op Ok
- write: check write_fails flag, if true return Err(ClawError::Io(BrokenPipe)), else push to sent
- messages: take receiver from Option (panics if called twice)
- end_input: no-op Ok
- close: no-op Ok
- is_ready: load from AtomicBool

FILE: tests/common/mod.rs (NEW)

pub mod mock_transport;
pub use mock_transport::MockTransport;

Helper functions:
- create_mock_control() -> (Arc<ControlProtocol>, Arc<MockTransport>)
- simulate_response(control, transport, response): waits 10ms, reads last sent msg,
  extracts request_id, calls handle_response
ENDDESIGN

TASK_1=$(bd create --title='Create shared test infrastructure - MockTransport and common utilities' --type=task --priority=1 \
  --description='Extract the MockTransport from control/mod.rs tests into a reusable tests/common/ module. Create tests/common/mod.rs and tests/common/mock_transport.rs with configurable mock transport supporting write failure injection, message simulation, and ready state control. Also add create_mock_control and simulate_response helpers.' \
  --acceptance='1. tests/common/mock_transport.rs exists with MockTransport struct implementing Transport trait. 2. MockTransport supports: capturing writes via get_sent/get_sent_json, injecting messages via inject_message/sender, toggling write failures via set_write_fails, controlling ready state via AtomicBool. 3. tests/common/mod.rs exports MockTransport and helper functions create_mock_control and simulate_response. 4. Existing tests in control/mod.rs still compile and pass (they can keep their local MockTransport or be refactored to use the shared one). 5. cargo test compiles cleanly with new files.' \
  --design="$(cat /tmp/bd_design_1.txt)" \
  --notes='Design decision: tests/common/ pattern rather than pub(crate) in src/ because this is test-only code. The existing MockTransport in control/mod.rs is well-designed but locked inside the test module. Do NOT add mockall or other mocking framework dependencies -- the hand-written mock gives precise control over failure injection. messages() can only be called once (takes from Option).' \
  --silent)
rm -f /tmp/bd_design_1.txt

# ─────────────────────────────────────────────────────────────────────
# Task 2: New Test Fixture Files
# ─────────────────────────────────────────────────────────────────────

cat <<'ENDDESIGN' > /tmp/bd_design_2.txt
FILE: crates/rusty_claw/tests/fixtures/control_request.ndjson (NEW)
Contents (4 lines):
{"type":"system","subtype":"init","session_id":"sess_ctrl_001","tools":[],"mcp_servers":[]}
{"type":"control_request","request_id":"req_001","request":{"subtype":"can_use_tool","tool_name":"Bash","tool_input":{"command":"ls"}}}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Done"}],"model":"claude-sonnet-4","id":"msg_1","stop_reason":"end_turn","usage":{"input_tokens":10,"output_tokens":5}}}
{"type":"result","subtype":"success","num_turns":1,"is_error":false,"session_id":"sess_ctrl_001"}

FILE: crates/rusty_claw/tests/fixtures/malformed_lines.ndjson (NEW)
Mix of valid JSON and garbage text. Example:
{"type":"system","subtype":"init","session_id":"sess_bad","tools":[],"mcp_servers":[]}
this is not valid json at all
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"hello"}],"model":"claude-sonnet-4","id":"msg_1","stop_reason":"end_turn","usage":{"input_tokens":1,"output_tokens":1}}}
{broken json {{{
{"type":"result","subtype":"success","num_turns":1,"is_error":false,"session_id":"sess_bad"}

FILE: crates/rusty_claw/tests/fixtures/rapid_burst.ndjson (NEW)
100 small assistant messages with minimal content for throughput testing.
Generate programmatically: system init line, then 100 assistant messages with incrementing IDs,
then a result line. Each message is small: {"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"msg N"}],...}}

FILE: large_message.ndjson
Generate programmatically during test setup rather than checking in a large file.
Create a ~100KB text field in a single assistant message JSON line.
ENDDESIGN

TASK_2=$(bd create --title='Create new test fixture files for integration tests' --type=task --priority=2 \
  --description='Create new NDJSON fixture files needed by integration tests: control_request.ndjson with control protocol messages, malformed_lines.ndjson with invalid JSON mixed in, and rapid_burst.ndjson with 100+ messages. The large_message fixture should be generated programmatically in test setup.' \
  --acceptance='1. control_request.ndjson exists in tests/fixtures/ with 4 lines: system init, control_request, assistant message, result. 2. malformed_lines.ndjson has mix of valid JSON lines and garbage text (at least 2 valid, 2 invalid). 3. rapid_burst.ndjson has 100+ small assistant messages plus system init and result lines. 4. All fixture files are valid NDJSON where expected (each valid line parses as JSON independently). 5. No large_message.ndjson checked in -- document that it should be generated in test setup.' \
  --design="$(cat /tmp/bd_design_2.txt)" \
  --notes='Existing fixtures: simple_query.ndjson, tool_use.ndjson, error_response.ndjson, thinking_content.ndjson. The mock_cli loads fixtures and outputs them line by line. For malformed_lines, the mock_cli may need enhancement or a separate approach since it currently loads fixtures as JSON. Consider using raw file output mode.' \
  --silent)
rm -f /tmp/bd_design_2.txt

# ─────────────────────────────────────────────────────────────────────
# Task 3: Transport NDJSON Reader Tests
# ─────────────────────────────────────────────────────────────────────

cat <<'ENDDESIGN' > /tmp/bd_design_3.txt
FILE: tests/integration_test.rs (add tests)

Tests exercise spawn_reader_task() through the public connect() + messages() API
using mock CLI to produce controlled output.

Test 1: test_transport_reads_valid_json_messages
- Spawn mock CLI with simple_query.ndjson fixture, --delay=0
- Connect transport, take messages receiver
- Collect all messages via rx.recv().await loop
- Assert: exactly 3 messages, all Ok(Value), first has "type":"system", order preserved

Test 2: test_reader_empty_lines_skipped
- Need fixture with empty lines between valid JSON OR unit-level test with mock stdout
- Assert: only non-empty JSON lines received, no errors for empty lines
- Implementation note: mock_cli may skip empty lines when loading, may need raw output mode

Test 3: test_reader_malformed_json_produces_error
- Use malformed_lines.ndjson fixture
- Assert: valid lines produce Ok(Value), invalid lines produce Err(ClawError::JsonDecode(_))
- Assert: reader continues after malformed lines (does not stop)

Test 4: test_transport_channel_closes_on_process_exit
- Spawn mock CLI with simple_query.ndjson, --delay=0
- Connect, assert is_ready() true
- Drain all messages via rx.recv().await loop
- After drain, rx.recv().await returns None
- Sleep 100ms for connected flag to update
- Assert: is_ready() returns false

Test 5: test_reader_large_message
- Generate large_message.ndjson programmatically (~100KB text field)
- Assert: message received intact without truncation, parsed JSON matches original

Test 6: test_reader_rapid_burst
- Use rapid_burst.ndjson fixture with 100+ messages, --delay=0
- Assert: all messages received, none dropped, order preserved
ENDDESIGN

TASK_3=$(bd create --title='Add transport NDJSON reader tests' --type=task --priority=2 \
  --description='Add 6 integration tests for the NDJSON reader task in tests/integration_test.rs. Tests cover: valid JSON parsing, empty line skipping, malformed JSON error handling, channel close on stdout end, large message handling, and rapid burst throughput. All tests use the mock CLI subprocess.' \
  --acceptance='1. test_transport_reads_valid_json_messages: receives exactly 3 Ok messages from simple_query.ndjson, first has type=system. 2. test_reader_empty_lines_skipped: only non-empty JSON lines received. 3. test_reader_malformed_json_produces_error: valid lines Ok, invalid lines Err(JsonDecode), reader continues. 4. test_transport_channel_closes_on_process_exit: rx.recv returns None after drain, is_ready returns false. 5. test_reader_large_message: ~100KB message received intact. 6. test_reader_rapid_burst: all 100+ messages received in order. All tests pass with cargo test --test integration_test.' \
  --design="$(cat /tmp/bd_design_3.txt)" \
  --notes='REQ-T1 traceability. Design decision: test spawn_reader_task through connect()+messages() rather than calling directly -- tests real code path without making function public. The mock CLI is the controlled stdout. Performance budget: each integration test under 2s.' \
  --silent)
rm -f /tmp/bd_design_3.txt

# ─────────────────────────────────────────────────────────────────────
# Task 4: Transport Write Tests
# ─────────────────────────────────────────────────────────────────────

cat <<'ENDDESIGN' > /tmp/bd_design_4.txt
FILE: src/transport/subprocess.rs (add to #[cfg(test)] mod tests)

Test 1: test_write_to_connected_transport
- Connect to mock CLI with simple_query.ndjson, --delay=100 (keep alive)
- Write a JSON message: {"type":"user","message":"hello"} + newline
- Assert: write() returns Ok(()), transport.is_ready() still true

Test 2: test_write_after_end_input_fails
- Connect to mock CLI
- Call end_input() -- succeeds
- Try write() -- returns error (ClawError::Connection for "stdin already closed"
  or ClawError::Io for broken pipe)
- Assert the error type matches one of those two

Test 3: test_concurrent_writes_dont_interleave
- Connect to mock CLI with --delay=200
- Write 10 messages sequentially (Transport::write takes &self so Mutex serializes)
- Assert: all writes return Ok(()), no panics
- Note: true concurrent testing would need Arc, but sequential writes through
  the Mutex still verify the lock works

Design decision: Write tests use mock CLI which does not read stdin but keeps pipe
open until exit. We verify: write succeeds, write errors after end_input, concurrent
writes do not panic. We cannot easily verify what was written without mock CLI echoing
stdin to stderr.
ENDDESIGN

TASK_4=$(bd create --title='Add transport write tests' --type=task --priority=2 \
  --description='Add 3 tests for transport write operations in src/transport/subprocess.rs. Tests cover: successful write to connected transport, write failure after end_input, and concurrent write safety through the Mutex.' \
  --acceptance='1. test_write_to_connected_transport: write returns Ok, is_ready remains true. 2. test_write_after_end_input_fails: end_input succeeds, subsequent write returns Err (ClawError::Connection or ClawError::Io). 3. test_concurrent_writes_dont_interleave: 10 sequential writes all return Ok, no panics, transport stays valid. All pass with cargo test --lib transport.' \
  --design="$(cat /tmp/bd_design_4.txt)" \
  --notes='REQ-T2 traceability. Mock CLI keeps stdin pipe open but does not read from it. Use --delay=100 or --delay=200 to keep process alive during writes. Transport::write takes &self so Mutex serializes concurrent access.' \
  --silent)
rm -f /tmp/bd_design_4.txt

# ─────────────────────────────────────────────────────────────────────
# Task 5: Process Lifecycle Tests
# ─────────────────────────────────────────────────────────────────────

cat <<'ENDDESIGN' > /tmp/bd_design_5.txt
FILE: src/transport/subprocess.rs (unit tests) + tests/integration_test.rs

Test 1: test_graceful_shutdown_normal_exit (subprocess.rs)
- Connect to mock CLI with short fixture, drain messages, call close()
- Assert: close() returns Ok(()), is_ready() returns false

Test 2: test_close_idempotent (subprocess.rs)
- Connect, drain messages, close twice
- Assert: first close Ok, second close Ok (idempotent), is_ready false
- Code:
  let mut rx = transport.messages();
  while let Some(_) = rx.recv().await {}
  assert!(transport.close().await.is_ok());
  assert!(transport.close().await.is_ok());
  assert!(!transport.is_ready());

Test 3: test_double_connect_rejected (subprocess.rs)
- Enhance existing test to use mock CLI instead of real claude binary
- Connect to mock CLI, try connect again
- Assert: first connect Ok, second returns ClawError::Connection("already connected")

Test 4: test_monitor_task_detects_unexpected_exit (integration_test.rs)
- Connect to mock CLI with very short fixture
- After fixture completes, process exits
- Wait for is_ready() to become false
- Assert: no panic or hang, is_ready eventually false
ENDDESIGN

TASK_5=$(bd create --title='Add process lifecycle tests' --type=task --priority=2 \
  --description='Add 4 tests for transport process lifecycle: graceful shutdown, idempotent close, double connect rejection, and unexpected exit detection. Tests go in subprocess.rs unit tests and integration_test.rs.' \
  --acceptance='1. test_graceful_shutdown_normal_exit: close returns Ok, is_ready false. 2. test_close_idempotent: two close calls both return Ok, is_ready false. 3. test_double_connect_rejected: second connect returns ClawError::Connection("already connected"), uses mock CLI not real claude. 4. test_monitor_task_detects_unexpected_exit: after process exits, is_ready becomes false without panic or hang.' \
  --design="$(cat /tmp/bd_design_5.txt)" \
  --notes='REQ-T3 traceability. The existing test_double_connect test depends on claude being installed -- enhance it to use mock CLI for deterministic behavior. Use --delay=0 for tests that need process to exit quickly.' \
  --silent)
rm -f /tmp/bd_design_5.txt

# ─────────────────────────────────────────────────────────────────────
# Task 6: Discovery Edge Case Tests
# ─────────────────────────────────────────────────────────────────────

cat <<'ENDDESIGN' > /tmp/bd_design_6.txt
FILE: src/transport/discovery.rs (add to #[cfg(test)] mod tests)

Test 1: test_discovery_env_var_takes_precedence
- Set CLAUDE_CLI_PATH env var to current test executable path
- Call CliDiscovery::find(None)
- Assert: returns the env var path
- Cleanup: unset env var
- WARNING: modifying env vars can affect concurrent tests. Consider using
  serial test attribute or a mutex to protect env var access.

Test 2: test_common_locations_includes_homebrew
- Call common_locations()
- Assert: result contains PathBuf for "/opt/homebrew/bin/claude"

Test 3: test_validate_version_rejects_old_version
- Hard to test without a custom binary that outputs "1.9.9"
- Consider creating a tiny shell script in a temp dir that echoes "1.9.9"
- Or test the version parsing logic directly if extractable
- Assert: validate_version() returns ClawError::InvalidCliVersion
ENDDESIGN

TASK_6=$(bd create --title='Add CLI discovery edge case tests' --type=task --priority=3 \
  --description='Add 3 edge case tests for CLI discovery: environment variable precedence, homebrew path inclusion, and old version rejection. Tests go in src/transport/discovery.rs unit tests.' \
  --acceptance='1. test_discovery_env_var_takes_precedence: setting CLAUDE_CLI_PATH env var causes find(None) to return that path. 2. test_common_locations_includes_homebrew: common_locations() contains /opt/homebrew/bin/claude. 3. test_validate_version_rejects_old_version: binary returning version 1.9.9 gets ClawError::InvalidCliVersion.' \
  --design="$(cat /tmp/bd_design_6.txt)" \
  --notes='Env var test modifies global state -- use serial test attribute or mutex to prevent concurrent test interference. Version rejection test may need a tiny shell script or test binary that outputs old version string. Consider feasibility before implementing the version test.' \
  --silent)
rm -f /tmp/bd_design_6.txt

# ─────────────────────────────────────────────────────────────────────
# Task 7: Control Module Timeout Tests
# ─────────────────────────────────────────────────────────────────────

cat <<'ENDDESIGN' > /tmp/bd_design_7.txt
FILE: src/control/mod.rs (add to #[cfg(test)] mod tests)

Uses tokio::time::pause() for deterministic time control. The "full" tokio feature
includes test-util. Call pause() BEFORE any time-dependent operations.

Test 1: test_request_timeout_returns_control_timeout
- tokio::time::pause()
- Create ControlProtocol with MockTransport
- Spawn request task (no response will come)
- tokio::time::advance(Duration::from_secs(31))
- Assert: result is Err(ClawError::ControlTimeout { subtype: "control_request" })
- Code:
  let handle = tokio::spawn(async move {
      control.request(ControlRequest::Interrupt).await
  });
  tokio::time::advance(Duration::from_secs(31)).await;
  let result = handle.await.unwrap();
  assert!(matches!(result.unwrap_err(), ClawError::ControlTimeout { .. }));

Test 2: test_timeout_cleans_up_pending_request
- Same setup as above
- After timeout, verify pending request is cleaned up
- Indirect verification: calling handle_response with the timed-out request_id
  should return false from complete() (entry no longer exists)
- Note: PendingRequests::len() is #[cfg(test)] -- can use it if accessible,
  but it is a private field of ControlProtocol. Verify indirectly.

Test 3: test_timeout_doesnt_affect_other_requests
- Send two requests concurrently
- Let one time out (no response)
- Respond to the other via handle_response
- Assert: first request gets ControlTimeout, second gets its response correctly
ENDDESIGN

TASK_7=$(bd create --title='Add control protocol timeout tests' --type=task --priority=2 \
  --description='Add 3 timeout tests for ControlProtocol using tokio::time::pause() for deterministic time control. Tests cover: basic timeout returning ControlTimeout error, cleanup of pending requests after timeout, and isolation ensuring one timeout does not affect other in-flight requests.' \
  --acceptance='1. test_request_timeout_returns_control_timeout: request with no response returns Err(ClawError::ControlTimeout { subtype: "control_request" }) after 31s advance. 2. test_timeout_cleans_up_pending_request: after timeout, the request_id is no longer in pending map (verify indirectly). 3. test_timeout_doesnt_affect_other_requests: one request times out while another receives its correct response.' \
  --design="$(cat /tmp/bd_design_7.txt)" \
  --notes='REQ-C1 traceability. IMPORTANT: tokio::time::pause() must be called before any time-dependent operations. Wall-clock time should be under 1s since time is simulated. The tokio "full" feature includes test-util. PendingRequests::len() is cfg(test) but private to ControlProtocol -- verify cleanup indirectly by checking handle_response returns false for timed-out ID.' \
  --silent)
rm -f /tmp/bd_design_7.txt

# ─────────────────────────────────────────────────────────────────────
# Task 8: Control Module Write Failure Tests
# ─────────────────────────────────────────────────────────────────────

cat <<'ENDDESIGN' > /tmp/bd_design_8.txt
FILE: src/control/mod.rs (add to #[cfg(test)] mod tests)

Test 1: test_request_write_failure_returns_connection_error
- Create ControlProtocol with MockTransport
- transport.set_write_fails(true)
- Call control.request(ControlRequest::Interrupt)
- Assert: returns Err(ClawError::Connection(_))
- Assert: error message contains "Failed to send control request"
- Code:
  let transport = Arc::new(MockTransport::new());
  transport.set_write_fails(true);
  let control = ControlProtocol::new(transport.clone() as Arc<dyn Transport>);
  let result = control.request(ControlRequest::Interrupt).await;
  assert!(matches!(result.unwrap_err(), ClawError::Connection(_)));

Test 2: test_request_after_write_failure_still_works
- Set write_fails true, send request (fails)
- Set write_fails false, send another request
- Spawn background task to simulate response for second request
- Assert: first request fails with Connection error
- Assert: second request succeeds with correct response
ENDDESIGN

TASK_8=$(bd create --title='Add control protocol write failure tests' --type=task --priority=2 \
  --description='Add 2 tests for transport write failure handling in ControlProtocol. Tests verify that write failures surface as ClawError::Connection and that the protocol recovers after a write failure.' \
  --acceptance='1. test_request_write_failure_returns_connection_error: request returns Err(ClawError::Connection(_)), message contains "Failed to send control request". 2. test_request_after_write_failure_still_works: first request fails with Connection, second request succeeds after re-enabling writes.' \
  --design="$(cat /tmp/bd_design_8.txt)" \
  --notes='REQ-C2 traceability. Uses MockTransport with set_write_fails. The write failure wraps the transport Io error into a Connection error with descriptive message. Recovery test needs a background task to simulate the response for the second request.' \
  --silent)
rm -f /tmp/bd_design_8.txt

# ─────────────────────────────────────────────────────────────────────
# Task 9: Control Module Concurrent Request Tests
# ─────────────────────────────────────────────────────────────────────

cat <<'ENDDESIGN' > /tmp/bd_design_9.txt
FILE: src/control/mod.rs (add to #[cfg(test)] mod tests)

Test: test_concurrent_requests_receive_correct_responses
- Create ControlProtocol with MockTransport, wrap both in Arc
- Spawn response simulator task:
  - Wait 50ms for requests to be sent
  - Read all sent messages from transport.get_sent()
  - For each, extract request_id, call control.handle_response with unique data
- Send 5 concurrent requests via tokio::spawn
- Assert: all 5 succeed, no cross-contamination
- Code:
  let transport_clone = transport.clone();
  let control_clone = control.clone();
  tokio::spawn(async move {
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
  let mut handles = vec![];
  for _ in 0..5 {
      let c = control.clone();
      handles.push(tokio::spawn(async move {
          c.request(ControlRequest::McpStatus).await
      }));
  }
  for handle in handles {
      assert!(handle.await.unwrap().is_ok());
  }
ENDDESIGN

TASK_9=$(bd create --title='Add control protocol concurrent request tests' --type=task --priority=2 \
  --description='Add 1 test verifying multiple in-flight control requests are routed correctly to their respective callers. Sends 5 concurrent requests and simulates responses matched by request_id.' \
  --acceptance='1. test_concurrent_requests_receive_correct_responses: 5 concurrent requests all receive their correct unique responses, no cross-contamination between requests. All 5 handle.await.unwrap() return Ok.' \
  --design="$(cat /tmp/bd_design_9.txt)" \
  --notes='REQ-C3 traceability. The response simulator must wait for requests to be sent before reading them. Use 50ms sleep (acceptable in this context). Each response includes unique data to verify correct routing.' \
  --silent)
rm -f /tmp/bd_design_9.txt

# ─────────────────────────────────────────────────────────────────────
# Task 10: Control Handler Dispatch Edge Cases
# ─────────────────────────────────────────────────────────────────────

cat <<'ENDDESIGN' > /tmp/bd_design_10.txt
FILE: src/control/mod.rs (add to #[cfg(test)] mod tests)

Test 1: test_handle_incoming_hook_no_handler_registered
- Create ControlProtocol without registering any hook handler
- Call handle_incoming with a HookCallback request
- Assert: ControlResponse::Error sent back to transport
- Assert: error message mentions "No handler registered for hook_id"

Test 2: test_handle_incoming_mcp_no_handler_registered
- Create ControlProtocol without MCP handler
- Call handle_incoming with McpMessage request
- Assert: ControlResponse::Error sent to transport
- Assert: error mentions "No MCP message handler registered"

Test 3: test_handle_incoming_can_use_tool_handler_error
- Register a CanUseToolHandler that returns Err(ClawError::ToolExecution("test error"))
- Call handle_incoming with a CanUseTool request
- Assert: ControlResponse::Error sent with the error message
ENDDESIGN

TASK_10=$(bd create --title='Add control handler dispatch edge case tests' --type=task --priority=2 \
  --description='Add 3 tests for handler dispatch edge cases in ControlProtocol: missing hook handler, missing MCP handler, and handler returning an error. All test that appropriate ControlResponse::Error messages are sent back to the transport.' \
  --acceptance='1. test_handle_incoming_hook_no_handler_registered: sends ControlResponse::Error mentioning "No handler registered for hook_id". 2. test_handle_incoming_mcp_no_handler_registered: sends ControlResponse::Error mentioning "No MCP message handler registered". 3. test_handle_incoming_can_use_tool_handler_error: handler error wrapped in ControlResponse::Error with the error message.' \
  --design="$(cat /tmp/bd_design_10.txt)" \
  --notes='REQ-H3 traceability. These tests verify the error path when handlers are missing or fail. Check how handle_incoming currently constructs error responses to match assertions.' \
  --silent)
rm -f /tmp/bd_design_10.txt

# ─────────────────────────────────────────────────────────────────────
# Task 11: Pending Requests Edge Cases
# ─────────────────────────────────────────────────────────────────────

cat <<'ENDDESIGN' > /tmp/bd_design_11.txt
FILE: src/control/pending.rs (add to #[cfg(test)] mod tests)

Test 1: test_pending_insert_duplicate_id
- Insert two entries with the same ID
- Assert: len() is 1 (not 2)
- Complete the ID -- sends to the second sender (first is dropped)

Test 2: test_pending_stress_100_concurrent
- Spawn 100 tasks, each inserting and completing with unique IDs
- Assert: all 100 completions return true
- Assert: all 100 receivers get their response
- Assert: final len() is 0
ENDDESIGN

TASK_11=$(bd create --title='Add pending requests edge case tests' --type=task --priority=3 \
  --description='Add 2 edge case tests for PendingRequests: duplicate ID insertion behavior and stress test with 100 concurrent insert+complete operations.' \
  --acceptance='1. test_pending_insert_duplicate_id: len() is 1 after two inserts with same ID, completing sends to second sender. 2. test_pending_stress_100_concurrent: all 100 completions return true, all receivers get responses, final len() is 0.' \
  --design="$(cat /tmp/bd_design_11.txt)" \
  --notes='PendingRequests::len() is #[cfg(test)] which is fine for these in-module tests. These are direct unit tests of the pending request store.' \
  --silent)
rm -f /tmp/bd_design_11.txt

# ─────────────────────────────────────────────────────────────────────
# Task 12: MCP Server Edge Case Tests
# ─────────────────────────────────────────────────────────────────────

cat <<'ENDDESIGN' > /tmp/bd_design_12.txt
FILE: src/mcp_server.rs (add to #[cfg(test)] mod tests)

Test 1: test_tools_call_missing_params
- Send {"jsonrpc":"2.0","id":1,"method":"tools/call"} (no params)
- Assert: returns JSON-RPC error with "Missing params" message
- Code: let response = server.handle_jsonrpc(request).await;
  assert!(response.is_err() || response.unwrap()["error"].is_object());

Test 2: test_tools_call_missing_arguments_defaults_to_empty
- Register a tool that echoes its input
- Send call with name but no arguments field in params
- Assert: tool executes successfully, receives {} as arguments

Test 3: test_tools_call_missing_name
- Send {"jsonrpc":"2.0","id":1,"method":"tools/call","params":{}}
- Assert: returns error with "Missing tool name"

Test 4: test_handle_missing_method
- Send {"jsonrpc":"2.0","id":1}
- Assert: returns Err(ClawError::ControlError("Missing method field"))

Test 5: test_duplicate_tool_last_wins
- Register "dup" with handler A (response "A"), then "dup" with handler B (response "B")
- Assert: list_tools().len() == 1
- Call "dup" tool, assert response text is "B"
- Code:
  server.register_tool(SdkMcpTool::new("dup", "First", json!({}), handler_a));
  server.register_tool(SdkMcpTool::new("dup", "Second", json!({}), handler_b));
  assert_eq!(server.list_tools().len(), 1);

Test 6: test_empty_tool_list
- Create server with no tools registered
- Assert: list_tools() returns empty vec
- Assert: tools/list JSON-RPC response has "tools": []

Test 7: test_registry_multiple_servers
- Register two servers with different tools
- Assert: messages routed to server A execute A tools, B execute B tools
- Assert: unknown server returns error

Test 8: test_null_json_rpc_id
- Send request with "id": null
- Assert: response includes "id": null
ENDDESIGN

TASK_12=$(bd create --title='Add MCP server edge case tests' --type=task --priority=2 \
  --description='Add 8 edge case tests for the MCP server module covering: missing params, missing arguments defaulting to empty, missing name, missing method, duplicate tool registration, empty tool list, multiple server routing, and null JSON-RPC id handling.' \
  --acceptance='1. test_tools_call_missing_params: JSON-RPC error for missing params. 2. test_tools_call_missing_arguments_defaults_to_empty: tool receives {} when arguments omitted. 3. test_tools_call_missing_name: error with "Missing tool name". 4. test_handle_missing_method: Err(ClawError::ControlError("Missing method field")). 5. test_duplicate_tool_last_wins: list_tools len 1, calling tool returns handler B response. 6. test_empty_tool_list: empty vec, "tools":[] in JSON-RPC. 7. test_registry_multiple_servers: correct routing per server, unknown returns error. 8. test_null_json_rpc_id: response has "id":null.' \
  --design="$(cat /tmp/bd_design_12.txt)" \
  --notes='REQ-M1 traceability. The current implementation does .as_object() on params which returns None for missing params. Need MockHandler struct for duplicate tool test. Existing test infrastructure in mcp_server.rs tests can be reused.' \
  --silent)
rm -f /tmp/bd_design_12.txt

# ─────────────────────────────────────────────────────────────────────
# Task 13: MCP Tool Content Serialization Tests
# ─────────────────────────────────────────────────────────────────────

cat <<'ENDDESIGN' > /tmp/bd_design_13.txt
FILE: src/mcp_server.rs (add to #[cfg(test)] mod tests)

Test 1: test_tool_content_text_serialization_roundtrip
- Create ToolContent::Text with text "hello"
- Serialize to JSON, deserialize back
- Assert: {"type":"text","text":"hello"} round-trips correctly

Test 2: test_tool_content_image_serialization_roundtrip
- Create ToolContent::Image with data and mime_type
- Serialize to JSON
- Assert: serialized JSON contains "mimeType" (camelCase) NOT "mime_type"
- Verify serde rename attribute is working

Test 3: test_tool_result_error_serialization
- Create ToolResult::error("fail")
- Serialize to JSON
- Assert: contains "isError": true (camelCase, not "is_error")
ENDDESIGN

TASK_13=$(bd create --title='Add MCP tool content serialization tests' --type=task --priority=3 \
  --description='Add 3 serialization round-trip tests for MCP tool content types: text content, image content with camelCase mimeType, and error result with isError flag.' \
  --acceptance='1. test_tool_content_text_serialization_roundtrip: {"type":"text","text":"hello"} round-trips. 2. test_tool_content_image_serialization_roundtrip: JSON contains "mimeType" not "mime_type". 3. test_tool_result_error_serialization: ToolResult::error("fail") has "isError":true.' \
  --design="$(cat /tmp/bd_design_13.txt)" \
  --notes='These verify serde rename attributes are correct. camelCase is required by the MCP protocol spec.' \
  --silent)
rm -f /tmp/bd_design_13.txt

# ─────────────────────────────────────────────────────────────────────
# Task 14: HookMatcher Tests (options.rs)
# ─────────────────────────────────────────────────────────────────────

cat <<'ENDDESIGN' > /tmp/bd_design_14.txt
FILE: src/options.rs (add to #[cfg(test)] mod tests)

Currently ZERO tests for HookMatcher. Add 9 tests:

Test 1: test_hook_matcher_all_matches_any_tool
  let m = HookMatcher::all();
  assert!(m.matches("Bash"));
  assert!(m.matches("Read"));
  assert!(m.matches("Edit"));
  assert!(m.matches(""));
  assert!(m.matches("some_random_tool_name_12345"));

Test 2: test_hook_matcher_tool_exact_match
  let m = HookMatcher::tool("Bash");
  assert!(m.matches("Bash"));
  assert!(!m.matches("bash"));  // case-sensitive
  assert!(!m.matches("Read"));
  assert!(!m.matches("Bash "));  // trailing space
  assert!(!m.matches(""));

Test 3: test_hook_matcher_tool_case_sensitive
  let m = HookMatcher::tool("Bash");
  assert!(m.matches("Bash"));
  assert!(!m.matches("bash"));
  assert!(!m.matches("BASH"));
  assert!(!m.matches("bAsH"));

Test 4: test_hook_matcher_all_has_none_tool_name
  let m = HookMatcher::all();
  assert!(m.tool_name.is_none());

Test 5: test_hook_matcher_tool_stores_name
  let m = HookMatcher::tool("Bash");
  assert_eq!(m.tool_name, Some("Bash".to_string()));

Test 6: test_hook_matcher_serialization_all
  let m = HookMatcher::all();
  let json = serde_json::to_value(&m).unwrap();
  assert!(!json.as_object().unwrap().contains_key("tool_name"));

Test 7: test_hook_matcher_serialization_tool
  let m = HookMatcher::tool("Bash");
  let json = serde_json::to_value(&m).unwrap();
  assert_eq!(json["tool_name"], "Bash");

Test 8: test_hook_matcher_deserialization_roundtrip
  // Test both variants round-trip through JSON
  let all = HookMatcher::all();
  let json = serde_json::to_string(&all).unwrap();
  let parsed: HookMatcher = serde_json::from_str(&json).unwrap();
  assert!(parsed.matches("anything"));
  let tool = HookMatcher::tool("Read");
  let json = serde_json::to_string(&tool).unwrap();
  let parsed: HookMatcher = serde_json::from_str(&json).unwrap();
  assert!(parsed.matches("Read"));
  assert!(!parsed.matches("Write"));

Test 9: test_hook_matcher_empty_string_tool
  let m = HookMatcher::tool("");
  assert!(m.matches(""));
  assert!(!m.matches("Bash"));
ENDDESIGN

TASK_14=$(bd create --title='Add HookMatcher tests in options.rs' --type=task --priority=2 \
  --description='Add 9 tests for HookMatcher which currently has ZERO test coverage. Tests cover: all() matches any tool, tool() exact match, case sensitivity, internal state (tool_name field), serialization for both variants, deserialization round-trip, and empty string edge case.' \
  --acceptance='1. test_hook_matcher_all_matches_any_tool: matches Bash, Read, Edit, empty string, random string. 2. test_hook_matcher_tool_exact_match: matches "Bash", rejects "bash", "Read", "Bash ", "". 3. test_hook_matcher_tool_case_sensitive: matches "Bash", rejects "bash", "BASH", "bAsH". 4. test_hook_matcher_all_has_none_tool_name: tool_name is None. 5. test_hook_matcher_tool_stores_name: tool_name is Some("Bash"). 6. test_hook_matcher_serialization_all: JSON has no tool_name key. 7. test_hook_matcher_serialization_tool: JSON has tool_name="Bash". 8. test_hook_matcher_deserialization_roundtrip: both variants survive serialize/deserialize. 9. test_hook_matcher_empty_string_tool: matches "" only.' \
  --design="$(cat /tmp/bd_design_14.txt)" \
  --notes='REQ-H1 traceability. All tests go in src/options.rs in the #[cfg(test)] mod tests section. HookMatcher is defined in options.rs. These are pure unit tests -- fast, no I/O, no async.' \
  --silent)
rm -f /tmp/bd_design_14.txt

# ─────────────────────────────────────────────────────────────────────
# Task 15: HookEvent Variant Tests (options.rs)
# ─────────────────────────────────────────────────────────────────────

cat <<'ENDDESIGN' > /tmp/bd_design_15.txt
FILE: src/options.rs (add to #[cfg(test)] mod tests)

Test 1: test_hook_event_all_variants_serialize
- Test all 10 variants serialize to expected strings:
  PreToolUse, PostToolUse, PostToolUseFailure, UserPromptSubmit, Stop,
  SubagentStop, SubagentStart, PreCompact, Notification, PermissionRequest
- Use vec of (variant, expected_string) tuples
- Assert: serde_json::to_value(&event).as_str() == expected for each

Test 2: test_hook_event_all_variants_roundtrip
- For each variant: from_value(to_value(v)) == v
- Requires PartialEq on HookEvent (should already be derived)

Test 3: test_hook_event_as_hashmap_key
- Tests Hash + Eq derive
  let mut map = HashMap::new();
  map.insert(HookEvent::PreToolUse, vec![HookMatcher::all()]);
  map.insert(HookEvent::PostToolUse, vec![HookMatcher::tool("Bash")]);
  assert_eq!(map.len(), 2);
  assert!(map.contains_key(&HookEvent::PreToolUse));

Test 4: test_hook_event_unknown_string_deserialization
  let result = serde_json::from_str::<HookEvent>("\"UnknownEvent\"");
  assert!(result.is_err());
ENDDESIGN

TASK_15=$(bd create --title='Add HookEvent variant tests in options.rs' --type=task --priority=2 \
  --description='Add 4 tests for HookEvent covering: serialization of all 10 variants, round-trip serialize/deserialize, HashMap key usage (Hash+Eq), and unknown string deserialization rejection.' \
  --acceptance='1. test_hook_event_all_variants_serialize: all 10 variants serialize to correct strings (PreToolUse, PostToolUse, PostToolUseFailure, UserPromptSubmit, Stop, SubagentStop, SubagentStart, PreCompact, Notification, PermissionRequest). 2. test_hook_event_all_variants_roundtrip: from_value(to_value(v)) == v for all. 3. test_hook_event_as_hashmap_key: HashMap with 2 entries works correctly. 4. test_hook_event_unknown_string_deserialization: "UnknownEvent" produces error.' \
  --design="$(cat /tmp/bd_design_15.txt)" \
  --notes='REQ-H2 traceability. Tests go in src/options.rs. HookEvent needs Hash, Eq, PartialEq derives (should already exist). Pure unit tests, no async needed.' \
  --silent)
rm -f /tmp/bd_design_15.txt

# ─────────────────────────────────────────────────────────────────────
# Task 16: Hook Callback and Response Edge Cases
# ─────────────────────────────────────────────────────────────────────

cat <<'ENDDESIGN' > /tmp/bd_design_16.txt
FILE: src/hooks/callback.rs (add to tests)

Test 1: test_hook_callback_returns_error
- Create a callback that returns Err(ClawError::ToolExecution("test error"))
- Call it
- Assert: call() returns the error

Test 2: test_hook_callback_with_all_input_fields
- Create HookInput with all fields populated: tool_name, tool_input, tool_output,
  error, prompt, metadata
- Pass to callback, verify all fields accessible

FILE: src/hooks/response.rs (add to tests)

Test 3: test_hook_response_with_updated_input
  let response = HookResponse::allow("ok")
      .with_updated_input(json!({"command": "ls -la"}));
  let json = serde_json::to_value(&response).unwrap();
  assert_eq!(json["updated_input"]["command"], "ls -la");

Test 4: test_hook_response_deny_sets_continue_false
  let response = HookResponse::deny("blocked");
  assert!(!response.should_continue);
  let json = serde_json::to_value(&response).unwrap();
  assert_eq!(json["continue"], false);
ENDDESIGN

TASK_16=$(bd create --title='Add hook callback and response edge case tests' --type=task --priority=3 \
  --description='Add 4 tests across hooks/callback.rs and hooks/response.rs: callback error propagation, callback with all input fields, response with updated_input, and deny setting continue to false.' \
  --acceptance='1. test_hook_callback_returns_error: call() returns Err(ClawError::ToolExecution). 2. test_hook_callback_with_all_input_fields: all HookInput fields accessible. 3. test_hook_response_with_updated_input: JSON has updated_input.command = "ls -la". 4. test_hook_response_deny_sets_continue_false: should_continue is false, JSON has "continue":false.' \
  --design="$(cat /tmp/bd_design_16.txt)" \
  --notes='REQ-H3 traceability for callback tests. These are simple unit tests. Check the actual field names in HookResponse -- the spec says "continue" in JSON which may use serde rename from should_continue.' \
  --silent)
rm -f /tmp/bd_design_16.txt

# ─────────────────────────────────────────────────────────────────────
# Task 17: Error Module Tests
# ─────────────────────────────────────────────────────────────────────

cat <<'ENDDESIGN' > /tmp/bd_design_17.txt
FILE: src/error.rs (add to #[cfg(test)] mod tests)

Error Source Chain Tests:

Test 1: test_io_error_source_chain
  use std::error::Error;
  let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
  let claw_err: ClawError = io_err.into();
  let source = claw_err.source();
  assert!(source.is_some());
  assert!(source.unwrap().to_string().contains("file missing"));

Test 2: test_json_decode_error_source_chain
  use std::error::Error;
  let json_err = serde_json::from_str::<Value>("not json").unwrap_err();
  let claw_err: ClawError = json_err.into();
  assert!(claw_err.source().is_some());

Test 3: test_non_from_variants_have_no_source
  use std::error::Error;
  assert!(ClawError::CliNotFound.source().is_none());
  assert!(ClawError::Connection("test".into()).source().is_none());
  assert!(ClawError::ControlTimeout { subtype: "test".into() }.source().is_none());

Error Display/Debug Tests:

Test 4: test_error_debug_output_all_variants
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
      assert!(!debug.is_empty());
  }

Test 5: test_error_display_all_variants
- Each variant to_string() contains expected substrings
- No variant panics on to_string()

Cross-Module Error Propagation:

Test 6: test_transport_write_error_through_control_request
- Same as task 8 test but in error.rs to verify error TYPE is Connection not Io
- MockTransport fails write, control.request wraps as Connection

Test 7: test_json_decode_error_through_response_stream
- FILE: src/client.rs
- Create ResponseStream backed by channel
- Inject Err(ClawError::JsonDecode(...))
- Assert: stream yields Err(ClawError::JsonDecode(...))
ENDDESIGN

TASK_17=$(bd create --title='Add error module tests - source chains, display, propagation' --type=task --priority=2 \
  --description='Add 7 tests for the error module covering: source chain for Io and JsonDecode errors, no-source for non-from variants, Debug output for all variants, Display output for all variants, and cross-module error propagation through control request and response stream.' \
  --acceptance='1. test_io_error_source_chain: source() is Some, contains "file missing". 2. test_json_decode_error_source_chain: source() is Some. 3. test_non_from_variants_have_no_source: CliNotFound, Connection, ControlTimeout all return None. 4. test_error_debug_output_all_variants: all 8+ variants produce non-empty Debug output. 5. test_error_display_all_variants: all variants produce non-empty Display, contain expected substrings. 6. test_transport_write_error_through_control_request: write error surfaces as ClawError::Connection. 7. test_json_decode_error_through_response_stream: stream yields JsonDecode error.' \
  --design="$(cat /tmp/bd_design_17.txt)" \
  --notes='REQ-E1 and REQ-E2 traceability. test_transport_write_error_through_control_request overlaps with task 8 but focuses on verifying the error TYPE transformation (Io -> Connection). test_json_decode_error_through_response_stream goes in client.rs tests.' \
  --silent)
rm -f /tmp/bd_design_17.txt

# ─────────────────────────────────────────────────────────────────────
# Task 18: Integration Tests - Full Transport I/O and ResponseStream
# ─────────────────────────────────────────────────────────────────────

cat <<'ENDDESIGN' > /tmp/bd_design_18.txt
FILE: tests/integration_test.rs (add tests)

Full Transport I/O Tests:

Test 1: test_transport_connect_read_messages_close
- Full lifecycle: connect -> read messages -> close
- Connect to mock CLI with simple_query.ndjson
- Assert: connect() Ok, is_ready() true
- Receive all 3 fixture messages via messages()
- After drain, channel closes
- close() succeeds

Test 2: test_transport_connect_write_read
- Connect to mock CLI, write a message, read responses
- Assert: write succeeds, fixture messages still received, no deadlock

Test 3: test_transport_with_tool_use_fixture
- Use tool_use.ndjson fixture
- Assert: all messages received and parseable, tool use content blocks present

End-to-End ResponseStream Tests:

Test 4: test_response_stream_parses_fixture_messages
- Create ResponseStream backed by channel
- Inject raw JSON matching fixture format
- Assert: System -> Message::System, Assistant -> Message::Assistant, Result -> Message::Result

Test 5: test_response_stream_filters_control_messages
  FILE: src/client.rs
  use tokio_stream::StreamExt;
  let (tx, rx) = mpsc::unbounded_channel();
  let mock_transport = Arc::new(MockTransport::new());
  let control = Arc::new(ControlProtocol::new(mock_transport));
  let mut stream = ResponseStream::new(rx, control);
  // Inject assistant message, drop tx
  // Assert: stream yields the assistant message
  // Control messages should be filtered out

Test 6: test_response_stream_completion
  FILE: src/client.rs
  - Create stream, drop sender
  - After next() returns None, is_complete() returns true
ENDDESIGN

TASK_18=$(bd create --title='Add integration and ResponseStream tests' --type=task --priority=2 \
  --description='Add 6 tests covering full transport I/O lifecycle and ResponseStream behavior: connect-read-close cycle, concurrent write-read, tool_use fixture parsing, ResponseStream message parsing, control message filtering, and stream completion detection.' \
  --acceptance='1. test_transport_connect_read_messages_close: full lifecycle works, all 3 messages received. 2. test_transport_connect_write_read: write + read without deadlock. 3. test_transport_with_tool_use_fixture: tool_use messages parse with content blocks. 4. test_response_stream_parses_fixture_messages: correct Message variant for each type. 5. test_response_stream_filters_control_messages: control messages not yielded, assistant messages yielded. 6. test_response_stream_completion: is_complete true after channel closes.' \
  --design="$(cat /tmp/bd_design_18.txt)" \
  --notes='REQ-C4 and REQ-I1 traceability. ResponseStream tests 5 and 6 go in src/client.rs tests, not integration_test.rs. Integration tests depend on mock_cli binary being built (env!("CARGO_BIN_EXE_mock_cli")). Performance budget: each integration test under 2s.' \
  --silent)
rm -f /tmp/bd_design_18.txt

# ─────────────────────────────────────────────────────────────────────
# Dependencies
# ─────────────────────────────────────────────────────────────────────

# All tasks are part of the epic
bd dep add "$EPIC_ID" "$TASK_1"
bd dep add "$EPIC_ID" "$TASK_2"
bd dep add "$EPIC_ID" "$TASK_3"
bd dep add "$EPIC_ID" "$TASK_4"
bd dep add "$EPIC_ID" "$TASK_5"
bd dep add "$EPIC_ID" "$TASK_6"
bd dep add "$EPIC_ID" "$TASK_7"
bd dep add "$EPIC_ID" "$TASK_8"
bd dep add "$EPIC_ID" "$TASK_9"
bd dep add "$EPIC_ID" "$TASK_10"
bd dep add "$EPIC_ID" "$TASK_11"
bd dep add "$EPIC_ID" "$TASK_12"
bd dep add "$EPIC_ID" "$TASK_13"
bd dep add "$EPIC_ID" "$TASK_14"
bd dep add "$EPIC_ID" "$TASK_15"
bd dep add "$EPIC_ID" "$TASK_16"
bd dep add "$EPIC_ID" "$TASK_17"
bd dep add "$EPIC_ID" "$TASK_18"

# Task dependencies: shared infrastructure blocks most other tasks
# Task 1 (MockTransport) blocks tasks that use it
bd dep add "$TASK_7" "$TASK_1"
bd dep add "$TASK_8" "$TASK_1"
bd dep add "$TASK_9" "$TASK_1"
bd dep add "$TASK_10" "$TASK_1"
bd dep add "$TASK_17" "$TASK_1"
bd dep add "$TASK_18" "$TASK_1"

# Task 2 (fixtures) blocks integration tests that use new fixtures
bd dep add "$TASK_3" "$TASK_2"

echo "$EPIC_ID"

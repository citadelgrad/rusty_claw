# Investigation: rusty_claw-isy - Add integration tests with mock CLI

**Task ID:** rusty_claw-isy
**Priority:** P2 (High)
**Status:** IN_PROGRESS
**Date:** 2026-02-13

## Executive Summary

This task requires creating **integration tests** for the Rusty Claw SDK using a **mock CLI binary** that replays canned NDJSON responses. Unlike unit tests (which mock individual components), integration tests verify end-to-end behavior by simulating a complete Claude CLI subprocess.

## Current State Analysis

### ✅ What Already Exists

**1. NDJSON Fixtures (4 files)** - Located in `crates/rusty_claw/tests/fixtures/`:
- `simple_query.ndjson` - Basic query with success result
- `tool_use.ndjson` - Multi-turn interaction with tool use
- `error_response.ndjson` - Error handling scenario
- `thinking_content.ndjson` - Thinking blocks example

**2. Comprehensive Unit Tests** - 184 tests covering:
- Transport layer (SubprocessCLITransport)
- Message parsing and serialization
- Control Protocol request/response handling
- ClaudeClient lifecycle (without real CLI)
- Hook system
- Permission management
- MCP server bridge

**3. Test Infrastructure:**
- `tests/` directory structure exists
- `fixtures/` directory with NDJSON files
- tokio test utilities in dev-dependencies
- MockTransport pattern (in control/mod.rs tests)

### ❌ What Needs to Be Built

**1. Mock CLI Binary (`mock_cli.rs`):**
- Standalone binary that can be executed like the real `claude` CLI
- Reads a fixture path from command-line args or environment variable
- Replays NDJSON lines from the fixture to stdout
- Simulates realistic timing and behavior
- Handles signal interruption (SIGTERM, SIGINT)
- Implements minimal CLI interface (version check, help text)

**2. Integration Test Framework:**
- Helper functions to spawn mock CLI with fixtures
- Assertion helpers for message sequences
- Timeout and error handling utilities
- Test cleanup (process termination)

**3. Integration Tests (15-20 tests minimum):**
- **query() tests** (5-6 tests)
  - Simple query with text response
  - Query with tool use
  - Query with error response
  - Query with thinking blocks
  - Query timeout handling
  - Stream completion and cleanup

- **ClaudeClient lifecycle tests** (5-6 tests)
  - Connect and disconnect
  - Send message and receive response
  - Multiple message exchanges
  - Control operations (interrupt, set_model, etc.)
  - Handler registration and invocation
  - Session state management

- **Control Protocol handshake tests** (2-3 tests)
  - Initialization request/response
  - Request ID matching
  - Timeout handling

- **Hook invocation tests** (2-3 tests)
  - Hook registration and callback
  - Hook with control protocol
  - Multiple hook handlers

## Architecture Design

### Mock CLI Binary Design

```text
┌─────────────────────────────────────────────────────┐
│              mock_cli binary                        │
│                                                     │
│  1. Parse CLI args (--fixture=path)                │
│  2. Load NDJSON fixture from disk                  │
│  3. For each line:                                 │
│     - Parse JSON                                   │
│     - Write line to stdout                         │
│     - Flush stdout                                 │
│     - Sleep 10-50ms (realistic delay)              │
│  4. Exit with code 0                               │
│                                                     │
│  Special handling:                                 │
│  - SIGTERM/SIGINT → graceful exit                  │
│  - --version → print "mock-cli 2.0.0"              │
│  - --help → print usage text                       │
└─────────────────────────────────────────────────────┘
```

**Binary Location:**
- Source: `crates/rusty_claw/tests/mock_cli.rs` (test-only binary)
- Binary: `target/debug/mock_cli` (built via Cargo)

**Cargo.toml Configuration:**
```toml
[[test]]
name = "integration"
path = "tests/integration_test.rs"
harness = true

[[bin]]
name = "mock_cli"
path = "tests/mock_cli.rs"
test = false
```

### Integration Test Architecture

```text
┌────────────────────────────────────────────────────┐
│         Integration Test Suite                     │
│                                                    │
│  ┌──────────────────────────────────────────┐    │
│  │  Test Helper Functions                   │    │
│  │  • spawn_mock_cli(fixture_path)          │    │
│  │  • assert_message_sequence(stream, ...)  │    │
│  │  • timeout_after(duration, future)       │    │
│  │  • cleanup_mock_cli(handle)              │    │
│  └──────────────────────────────────────────┘    │
│                                                    │
│  ┌──────────────────────────────────────────┐    │
│  │  query() Integration Tests               │    │
│  │  • test_query_simple()                   │    │
│  │  • test_query_with_tool_use()            │    │
│  │  • test_query_error_response()           │    │
│  │  • test_query_thinking_blocks()          │    │
│  └──────────────────────────────────────────┘    │
│                                                    │
│  ┌──────────────────────────────────────────┐    │
│  │  ClaudeClient Integration Tests          │    │
│  │  • test_client_connect_disconnect()      │    │
│  │  • test_client_send_message()            │    │
│  │  • test_client_interrupt()               │    │
│  │  • test_client_control_operations()      │    │
│  └──────────────────────────────────────────┘    │
│                                                    │
│  ┌──────────────────────────────────────────┐    │
│  │  Control Protocol Integration Tests      │    │
│  │  • test_control_handshake()              │    │
│  │  • test_control_request_response()       │    │
│  └──────────────────────────────────────────┘    │
│                                                    │
│  ┌──────────────────────────────────────────┐    │
│  │  Hook Integration Tests                  │    │
│  │  • test_hook_invocation()                │    │
│  │  • test_multiple_hooks()                 │    │
│  └──────────────────────────────────────────┘    │
└────────────────────────────────────────────────────┘
         ↓ uses ↓
┌────────────────────────────────────────────────────┐
│         mock_cli binary                            │
│  (replays NDJSON fixtures)                         │
└────────────────────────────────────────────────────┘
```

## Implementation Plan

### Phase 1: Mock CLI Binary (90 minutes)

**Files to Create:**
1. `crates/rusty_claw/tests/mock_cli.rs` (~200 lines)

**Implementation Steps:**
1. Create binary skeleton with CLI arg parsing (clap or manual)
2. Implement fixture loading and validation
3. Add NDJSON replay loop with realistic delays
4. Add signal handling (SIGTERM, SIGINT)
5. Add --version and --help flags
6. Test manually with existing fixtures

**Key Requirements:**
- Exit code 0 on success, non-zero on error
- Flush stdout after each line (required for NDJSON streaming)
- Sleep 10-50ms between messages (simulate real CLI)
- Handle broken pipe (client disconnect)
- Validate JSON before writing (fail early on malformed fixtures)

### Phase 2: Integration Test Helpers (60 minutes)

**Files to Create:**
1. `crates/rusty_claw/tests/integration_test.rs` (~150 lines helpers)

**Helper Functions:**
```rust
// Spawn mock CLI with fixture
async fn spawn_mock_cli(fixture_name: &str) -> Result<(SubprocessCLITransport, Child), ClawError>

// Assert message sequence matches expected types
fn assert_message_sequence(messages: Vec<Message>, expected: &[&str])

// Timeout helper
async fn timeout_after<F, T>(duration: Duration, future: F) -> Result<T, ClawError>
    where F: Future<Output = T>

// Cleanup helper
async fn cleanup_mock_cli(child: Child) -> Result<(), ClawError>

// Get fixture path
fn fixture_path(name: &str) -> PathBuf
```

### Phase 3: query() Integration Tests (90 minutes)

**Test Coverage:**
```rust
#[tokio::test]
async fn test_query_simple() {
    // Use simple_query.ndjson fixture
    // Verify: init → assistant → result
    // Verify: result contains success status
}

#[tokio::test]
async fn test_query_with_tool_use() {
    // Use tool_use.ndjson fixture
    // Verify: init → assistant (tool_use) → user (tool_result) → assistant → result
    // Verify: tool_use_id matches tool_result
}

#[tokio::test]
async fn test_query_error_response() {
    // Use error_response.ndjson fixture
    // Verify: init → assistant → result (error)
    // Verify: error message and code present
}

#[tokio::test]
async fn test_query_thinking_blocks() {
    // Use thinking_content.ndjson fixture
    // Verify: init → assistant (thinking + text) → result
    // Verify: thinking content parsed correctly
}

#[tokio::test]
async fn test_query_stream_completion() {
    // Verify stream ends after result message
    // Verify transport cleanup
}

#[tokio::test]
async fn test_query_with_options() {
    // Test query() with ClaudeAgentOptions
    // Verify CLI args are passed correctly
}
```

### Phase 4: ClaudeClient Integration Tests (90 minutes)

**Test Coverage:**
```rust
#[tokio::test]
async fn test_client_connect_disconnect() {
    // Create client, connect, verify is_connected(), disconnect
}

#[tokio::test]
async fn test_client_send_message() {
    // Connect, send_message(), verify response stream
}

#[tokio::test]
async fn test_client_multiple_messages() {
    // (Will fail initially - send_message() takes receiver)
    // Document limitation or implement fix
}

#[tokio::test]
async fn test_client_interrupt() {
    // Send control interrupt request
    // Verify response (using fixture with control_response)
}

#[tokio::test]
async fn test_client_set_model() {
    // Send set_model request, verify response
}

#[tokio::test]
async fn test_client_set_permission_mode() {
    // Send permission mode change, verify response
}

#[tokio::test]
async fn test_client_handler_registration() {
    // Register handlers before connect
    // Verify handlers are invoked (may need fixture with incoming requests)
}
```

**Note:** ClaudeClient tests may require creating additional fixtures with control protocol messages.

### Phase 5: Control Protocol Integration Tests (60 minutes)

**Test Coverage:**
```rust
#[tokio::test]
async fn test_control_handshake() {
    // Verify initialization request/response
    // Use fixture with system init message
}

#[tokio::test]
async fn test_control_request_response_matching() {
    // Send request, verify response has matching request_id
}

#[tokio::test]
async fn test_control_timeout() {
    // Send request with short timeout
    // Verify timeout error
    // (May need fixture that doesn't send response)
}
```

### Phase 6: Hook Integration Tests (60 minutes)

**Test Coverage:**
```rust
#[tokio::test]
async fn test_hook_invocation() {
    // Register hook before connect
    // Send message that triggers hook
    // Verify hook callback invoked with correct data
}

#[tokio::test]
async fn test_multiple_hooks() {
    // Register multiple hooks
    // Verify all are invoked in order
}

#[tokio::test]
async fn test_hook_with_control_protocol() {
    // Verify hook responses are sent via control protocol
}
```

**Note:** Hook tests require fixtures with `control_request` messages containing hook invocation data.

### Phase 7: Additional Fixtures (60 minutes)

**New Fixtures Needed:**
1. `control_interrupt.ndjson` - Control interrupt scenario
2. `control_set_model.ndjson` - Model change scenario
3. `hook_invocation.ndjson` - Hook callback scenario
4. `multi_turn.ndjson` - Multiple user/assistant exchanges

**Fixture Format Example (control_interrupt.ndjson):**
```json
{"type":"system","subtype":"init","session_id":"sess_001","tools":[],"mcp_servers":[]}
{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Starting long task..."}]}}
{"type":"control_request","request_id":"req_001","request":{"type":"interrupt"}}
{"type":"control_response","request_id":"req_001","response":{"type":"success","data":{"status":"interrupted"}}}
{"type":"result","subtype":"success","result":"Task interrupted","duration_ms":500,"num_turns":1,"usage":{"input_tokens":10,"output_tokens":5}}
```

### Phase 8: Verification and Documentation (60 minutes)

**Tasks:**
1. Run all integration tests: `cargo test --test integration`
2. Verify zero regressions: `cargo test --lib`
3. Run clippy: `cargo clippy --all-targets -- -D warnings`
4. Document test coverage in `.attractor/test-results.md`
5. Add README in `tests/` directory explaining integration test structure
6. Update task documentation

## Files to Create/Modify

### New Files (4 files, ~800 lines)

1. **`crates/rusty_claw/tests/mock_cli.rs`** (~200 lines)
   - Mock CLI binary implementation
   - Signal handling
   - NDJSON replay logic

2. **`crates/rusty_claw/tests/integration_test.rs`** (~400 lines)
   - Test helpers (~150 lines)
   - query() tests (~100 lines)
   - ClaudeClient tests (~150 lines)

3. **`crates/rusty_claw/tests/control_integration_test.rs`** (~100 lines)
   - Control Protocol tests
   - Hook invocation tests

4. **`crates/rusty_claw/tests/README.md`** (~100 lines)
   - Integration test documentation
   - How to add new tests
   - How to create fixtures

### New Fixtures (4 files)

5. **`crates/rusty_claw/tests/fixtures/control_interrupt.ndjson`**
6. **`crates/rusty_claw/tests/fixtures/control_set_model.ndjson`**
7. **`crates/rusty_claw/tests/fixtures/hook_invocation.ndjson`**
8. **`crates/rusty_claw/tests/fixtures/multi_turn.ndjson`**

### Modified Files (1 file, +15 lines)

9. **`crates/rusty_claw/Cargo.toml`**
   - Add [[bin]] section for mock_cli
   - Add [[test]] section for integration tests

## Acceptance Criteria Mapping

| # | Criterion | Implementation Plan |
|---|-----------|---------------------|
| 1 | Create mock_cli.rs binary | Phase 1 - Mock CLI Binary |
| 2 | Implement NDJSON fixture system | Phase 1 + Phase 7 (additional fixtures) |
| 3 | Write query() integration tests | Phase 3 - query() tests (5-6 tests) |
| 4 | Write ClaudeClient lifecycle tests | Phase 4 - ClaudeClient tests (5-6 tests) |
| 5 | Write control protocol handshake tests | Phase 5 - Control Protocol tests (2-3 tests) |
| 6 | Write hook invocation tests | Phase 6 - Hook tests (2-3 tests) |
| 7 | 15-20 integration tests | Total: ~18 tests across all phases |
| 8 | All tests pass, no regressions | Phase 8 - Verification (cargo test) |
| 9 | Zero clippy warnings | Phase 8 - Verification (cargo clippy) |

## Technical Considerations

### Mock CLI vs Real CLI

**Why mock CLI instead of real CLI?**
- **Deterministic:** Tests produce consistent results
- **Fast:** No network calls, no model inference
- **CI/CD friendly:** No API keys or external dependencies
- **Isolated:** Tests can run in parallel without interference
- **Error scenarios:** Easy to simulate edge cases and errors

### Subprocess Management

**Key Challenges:**
- Process cleanup (avoid zombies)
- Signal handling (graceful shutdown)
- Timeout handling (prevent hanging tests)
- Broken pipe handling (client disconnect)

**Solutions:**
- Use tokio::process::Command for async process spawning
- Track Child handles and kill in cleanup
- Use tokio::time::timeout for test timeouts
- Handle EPIPE errors gracefully in mock CLI

### NDJSON Streaming

**Requirements:**
- Each line must be valid JSON
- Lines must be newline-terminated
- Stdout must be flushed after each line
- Messages must arrive in order

**Mock CLI Implementation:**
```rust
for line in lines {
    // Parse to verify valid JSON
    let json: serde_json::Value = serde_json::from_str(&line)?;

    // Write to stdout with newline
    println!("{}", line);

    // Flush immediately (critical for streaming)
    std::io::stdout().flush()?;

    // Simulate realistic delay
    tokio::time::sleep(Duration::from_millis(20)).await;
}
```

### Transport Integration

**Using mock CLI with existing transport:**
```rust
// In integration test
let mock_cli_path = env!("CARGO_BIN_EXE_mock_cli"); // Cargo sets this
let args = vec![
    "--fixture=simple_query.ndjson".to_string(),
    "--output-format=stream-json".to_string(),
];

let mut transport = SubprocessCLITransport::new(
    Some(PathBuf::from(mock_cli_path)),
    args
);

transport.connect().await?;
// ... test transport behavior
```

## Dependencies & Risks

### Dependencies (All Satisfied ✅)

1. ✅ **rusty_claw-1ke** (Unit tests) - COMPLETE
   - Message parsing tests provide baseline
   - Fixtures already created

2. ✅ **rusty_claw-qrl** (ClaudeClient) - COMPLETE
   - ClaudeClient implementation exists
   - Ready for integration testing

3. ✅ **Existing Infrastructure:**
   - Transport layer (SubprocessCLITransport) - ✅
   - Control Protocol - ✅
   - Hook system - ✅
   - Message types - ✅
   - Error types - ✅

### Risks & Mitigations

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Mock CLI behavior differs from real CLI | High | Medium | Reference real CLI output, validate fixture format against spec |
| Process cleanup failures (zombies) | Medium | Low | Implement Drop handlers, use test cleanup functions |
| Test timeouts in CI | Medium | Medium | Use generous timeouts (5-10s), skip slow tests if needed |
| Fixture maintenance burden | Low | Medium | Document fixture format, provide fixture validation tool |
| Race conditions in async tests | Medium | Low | Use tokio::sync primitives, avoid shared mutable state |

## Success Metrics

**Quantitative:**
- ✅ 15-20 integration tests implemented
- ✅ 100% integration test pass rate
- ✅ Zero regressions (184/184 unit tests still pass)
- ✅ Zero clippy warnings
- ✅ All 9 acceptance criteria met

**Qualitative:**
- ✅ Tests are deterministic (same result every run)
- ✅ Tests are fast (<5s total runtime)
- ✅ Test code is maintainable (clear structure, helpers)
- ✅ Fixtures are reusable (documented format)
- ✅ Documentation explains integration test approach

## Estimated Timeline

| Phase | Description | Duration |
|-------|-------------|----------|
| 1 | Mock CLI Binary | 90 min |
| 2 | Test Helpers | 60 min |
| 3 | query() Tests | 90 min |
| 4 | ClaudeClient Tests | 90 min |
| 5 | Control Protocol Tests | 60 min |
| 6 | Hook Tests | 60 min |
| 7 | Additional Fixtures | 60 min |
| 8 | Verification & Documentation | 60 min |
| **Total** | | **570 min (9.5 hours)** |

## Example Test Structure

### Integration Test Example (query())

```rust
#[tokio::test]
async fn test_query_simple() {
    // Setup: Build mock CLI binary path
    let mock_cli = env!("CARGO_BIN_EXE_mock_cli");
    let fixture = fixture_path("simple_query.ndjson");

    // Arrange: Create transport with mock CLI
    let args = vec![
        format!("--fixture={}", fixture.display()),
        "--output-format=stream-json".to_string(),
    ];
    let mut transport = SubprocessCLITransport::new(
        Some(PathBuf::from(mock_cli)),
        args,
    );

    // Act: Connect and call query()
    transport.connect().await.unwrap();
    let mut stream = query("test prompt", None).await.unwrap();

    // Assert: Verify message sequence
    let mut messages = vec![];
    while let Some(msg) = stream.next().await {
        messages.push(msg.unwrap());
    }

    assert_eq!(messages.len(), 3); // init, assistant, result
    assert!(matches!(messages[0], Message::System(_)));
    assert!(matches!(messages[1], Message::Assistant(_)));
    assert!(matches!(messages[2], Message::Result(_)));

    // Verify result status
    if let Message::Result(result) = &messages[2] {
        assert_eq!(result.subtype, Some("success".to_string()));
    }
}
```

## Next Steps

1. ✅ Investigation complete
2. ⏭️ **Phase 1:** Implement mock CLI binary
3. ⏭️ **Phase 2:** Create integration test helpers
4. ⏭️ **Phase 3-6:** Implement test suites
5. ⏭️ **Phase 7:** Create additional fixtures
6. ⏭️ **Phase 8:** Verify and document
7. ⏭️ Commit and close task

---

**Investigation Status:** ✅ COMPLETE
**Ready to Proceed:** YES
**Blockers:** NONE

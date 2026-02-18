# TEST_PRD.md -- Product Requirements Document for Comprehensive Test Coverage

> rusty_claw test initiative -- the definitive planning document for achieving
> production-grade test coverage across the five core SDK modules.

## 1. Executive Summary

### 1.1 Why This Matters

rusty_claw is a Rust SDK that wraps the Claude Code CLI as a subprocess and communicates
with it over NDJSON-over-stdio. The SDK sits on the critical path between user code and
the Claude CLI -- every bug in the SDK is a bug in every application built on it.

The current test suite (116 tests) covers the **shape** of the codebase -- serialization
round-trips, constructor behavior, error message formatting -- but not the **behavior**.
The most important operations in the SDK are untested:

- **Transport message I/O** (the core reason the transport exists)
- **Control protocol timeouts** (the 30-second timeout logic is present but never exercised)
- **HookMatcher pattern matching** (ZERO tests for the `matches()`, `all()`, `tool()` methods)
- **Process lifecycle** (SIGTERM/SIGKILL shutdown path is entirely untested)
- **Cross-module error propagation** (no test verifies that a transport error surfaces correctly through control and up to the client)

These gaps mean the SDK could ship with silent regressions in its most important
functionality. A user could upgrade rusty_claw and discover that process shutdown hangs,
control requests silently time out, or hook matchers don't match -- none of which would
be caught by the existing test suite.

### 1.2 Scope

This initiative covers the five core modules of the `crates/rusty_claw/` crate:

| Module | Current Tests | Primary Gap |
|--------|--------------|-------------|
| `transport/` | 17 | Message I/O, process lifecycle, NDJSON parsing |
| `control/` | 38 | Timeout logic, transport failures, concurrent requests |
| `mcp_server` | 24 | Schema validation, concurrent execution, malformed JSON-RPC |
| `hooks/` | 18 | HookMatcher (0 tests), most HookEvent variants, execution order |
| `error` | 19 | Error propagation chains, source() chains, cross-module flow |

Additionally, this initiative includes **cross-cutting integration tests** that exercise
the full path: transport -> control -> mcp/hooks -> client response.

### 1.3 Out of Scope

- The `rusty_claw_macros` proc macro crate (has its own test needs)
- The `messages` module (well-tested via fixtures already)
- The `query` module (thin wrapper around client)
- The `permissions` module (small surface area, lower risk)
- Performance benchmarks (valuable but a separate initiative)
- Fuzz testing (valuable but a separate initiative)

---

## 2. Current State Assessment

### 2.1 What Exists and Works

The existing 116 tests provide solid coverage for:

**Serialization/deserialization correctness.** Every `ControlRequest` variant, every
`ControlResponse` variant, `IncomingControlRequest`, `HookInput`, `HookContext`,
`HookResponse`, `PermissionDecision`, `ToolContent`, `ToolResult`, and `AgentDefinition`
have round-trip serialization tests. This is valuable because the SDK communicates via
JSON, and serde attribute mistakes would be caught.

**Constructor and builder patterns.** `ClaudeAgentOptions::builder()` chains, `SubprocessCLITransport::new()`, `PendingRequests::new()`, `SdkMcpServerImpl::new()`,
`ControlHandlers::new()` -- all tested for correct initial state.

**Error message formatting.** Every `ClawError` variant is tested for its `Display`
output. The `From<io::Error>` and `From<serde_json::Error>` conversions are tested.

**Mock transport in control tests.** The `control/mod.rs` tests define a `MockTransport`
that captures sent bytes and allows simulating responses. This is a good pattern that
should be extracted and reused.

**Integration test infrastructure.** A `mock_cli` binary exists at `tests/mock_cli.rs`
that replays NDJSON fixture files. Four fixture files exist (`simple_query.ndjson`,
`tool_use.ndjson`, `error_response.ndjson`, `thinking_content.ndjson`). The integration
tests verify the mock CLI works and that messages can be parsed from fixtures.

### 2.2 What Is Missing (Risk Analysis)

**CRITICAL RISK -- Transport Message I/O (untested)**

The transport's `spawn_reader_task()` function is the entry point for all data from the
CLI. It reads lines from stdout, parses them as JSON, and sends them through an unbounded
channel. This function is never tested. The implications:

- A regression in NDJSON line splitting would silently corrupt all messages
- The empty-line skip behavior (`line.trim().is_empty()`) is untested
- The error path (malformed JSON sent to channel as `Err(ClawError::JsonDecode)`) is untested
- Channel closure when stdout ends is untested
- The `connected` flag being set to `false` when the reader exits is untested

Similarly, `spawn_stderr_task()` (captures stderr for diagnostics) and
`spawn_monitor_task()` (detects unexpected process exits) are untested.

The `write()` method is only tested for the "not connected" error case. Writing to a
connected transport, flushing, and error handling on write failure are untested.

**CRITICAL RISK -- Process Lifecycle (untested)**

`graceful_shutdown()` implements a multi-step shutdown:
1. Close stdin via `end_input()`
2. Wait up to 5 seconds for the process to exit
3. If timeout, call `force_shutdown()`

`force_shutdown()` implements SIGTERM -> wait 5s -> SIGKILL.

None of this is tested. A regression here means:
- Processes that don't exit on stdin close would hang forever
- The SIGTERM timeout could be wrong (too short, too long, or broken)
- SIGKILL might not be sent, leaving zombie processes
- On non-Unix platforms, the `child.kill()` fallback is untested

**HIGH RISK -- Control Protocol Timeouts (untested)**

The `request()` method has a 30-second timeout:
```rust
match tokio::time::timeout(Duration::from_secs(30), rx).await {
    Ok(Ok(response)) => Ok(response),
    Ok(Err(_)) => Err(ClawError::ControlError("Response channel closed")),
    Err(_) => { self.pending.cancel(&id).await; Err(ClawError::ControlTimeout { .. }) }
}
```

The timeout path is untested. The cancel cleanup is untested. If the pending entry
isn't cleaned up on timeout, it's a memory leak. If the timeout doesn't fire,
`request()` hangs forever.

**HIGH RISK -- HookMatcher (ZERO tests)**

`HookMatcher` has three methods: `all()`, `tool()`, and `matches()`. None are tested.
The doc comments show them being used and have expected behavior, but there are literally
zero test functions that call any of these methods. This is despite `HookMatcher` being
serialized and sent to the CLI during initialization -- if `matches()` is wrong, hooks
won't fire for the right events.

**MEDIUM RISK -- Concurrent Operations (untested)**

The codebase uses `Arc<Mutex<>>` extensively:
- `SubprocessCLITransport::stdin` -- `Arc<Mutex<Option<ChildStdin>>>`
- `SubprocessCLITransport::messages_rx` -- `Arc<std::sync::Mutex<Option<MessageReceiver>>>`
- `SubprocessCLITransport::connected` -- `Arc<AtomicBool>`
- `ControlProtocol::handlers` -- `Arc<Mutex<ControlHandlers>>`
- `PendingRequests::inner` -- `Arc<Mutex<HashMap<...>>>`

The only concurrency test is `test_concurrent_access` in `pending.rs` which spawns
10 tasks that insert-and-complete. There are no tests for:
- Concurrent writes to the transport
- Concurrent control requests
- Reading messages while writing
- Handler dispatch concurrent with request sending

**MEDIUM RISK -- Error Propagation Chains (untested)**

The error module tests verify message formatting but not propagation. For example:
- A broken pipe in `write()` should produce `ClawError::Io` -- but does the control
  protocol's `request()` re-wrap this as `ClawError::Connection`? Untested.
- Does `ClawError::Process` correctly carry the stderr buffer from the monitor task?
  Untested.
- Does `ClawError` implement `std::error::Error::source()` correctly for the `#[from]`
  variants? Untested.

**LOW RISK -- MCP Edge Cases (partially tested)**

The MCP module has decent test coverage for the happy path. Missing:
- `tools/call` with no `params` field
- `tools/call` with no `arguments` field (should default to `{}`)
- Missing `method` field entirely
- `handle_jsonrpc` with `null` id
- Registering a tool with the same name twice (last-write-wins behavior)
- Empty tool list response

### 2.3 Risk Summary Table

| Gap | Risk | Impact | Likelihood | Priority |
|-----|------|--------|-----------|----------|
| Transport message I/O | Critical | All messages corrupted silently | Medium | P0 |
| Process lifecycle | Critical | Zombie processes, hangs | Medium | P0 |
| Control timeout | High | Client hangs on CLI crash | High | P0 |
| HookMatcher (0 tests) | High | Hooks don't fire correctly | Medium | P1 |
| Concurrent operations | Medium | Data races, deadlocks | Low | P1 |
| Error propagation | Medium | Wrong error types to users | Medium | P2 |
| MCP edge cases | Low | Bad JSON-RPC responses | Low | P2 |
| HookEvent variants | Low | Serialization mismatches | Low | P3 |

---

## 3. Goals and Success Criteria

### 3.1 Quantitative Goals

| Metric | Current | Target | Rationale |
|--------|---------|--------|-----------|
| Total test count | 116 | ~220-250 | Roughly double, focused on behavior |
| Transport tests | 17 | ~45-50 | Most untested functionality lives here |
| Control tests | 38 | ~55-60 | Timeout, failure, concurrency tests |
| MCP tests | 24 | ~35-40 | Edge cases and concurrency |
| Hooks tests | 18 | ~35-40 | HookMatcher alone needs ~10 tests |
| Error tests | 19 | ~30-35 | Propagation and source chain tests |
| Integration tests | ~25 | ~35-45 | End-to-end through mock CLI |
| Tests using mock_cli | 7 | ~15-20 | Underutilized infrastructure |

<!-- Note: These numbers are estimates. The SPEC document defines exact test functions.
The important thing is not hitting a number but covering every identified gap. -->

### 3.2 Qualitative Goals

1. **Every public method has at least one test.** Currently, `HookMatcher::matches()`,
   `HookMatcher::all()`, `HookMatcher::tool()` are public with zero tests.

2. **Every error path is exercised.** The transport's write-failure path, the control
   protocol's timeout path, and the MCP server's missing-method path all need tests.

3. **Concurrency invariants are validated.** The `Arc<Mutex<>>` patterns should be
   tested under concurrent access to verify no panics, deadlocks, or data loss.

4. **The mock CLI infrastructure is fully utilized.** The mock CLI should be used for
   transport message I/O tests, process lifecycle tests, and full end-to-end integration
   tests. Currently it's only used for message parsing verification.

5. **Tests are deterministic and fast.** No tests should depend on the real `claude` CLI
   being installed. No tests should use `sleep()` for synchronization (use channels
   and barriers instead). The entire test suite should complete in under 30 seconds.

6. **Tests document invariants.** Each test should have a comment explaining what
   invariant it protects and what could go wrong if the test were removed.

### 3.3 Non-Goals

- 100% line coverage (diminishing returns; focus on behavior coverage)
- Testing internal private helpers directly (test through public API)
- Testing `Drop` implementations (hard to test deterministically)
- Testing logging output (tracing is observability, not correctness)

---

## 4. Prioritized Requirements

### 4.1 P0 -- Must Have (Blocking Release Confidence)

#### REQ-T1: Transport NDJSON Read Path

**Rationale:** This is the most critical untested code in the SDK. Every message
from the CLI flows through `spawn_reader_task()`. A bug here affects every user.

**Acceptance Criteria:**
- Test that valid JSON lines are parsed and sent through the channel
- Test that empty lines are skipped (not sent as errors)
- Test that malformed JSON lines produce `ClawError::JsonDecode` on the channel
- Test that the channel closes when stdout ends
- Test that `connected` flag is set to `false` when the reader task exits
- Test with multiple messages in rapid succession
- Test with large JSON messages (>64KB)

#### REQ-T2: Transport Write Path

**Rationale:** Writing messages to stdin is the other half of the transport's
core responsibility. Write failures need to be handled correctly.

**Acceptance Criteria:**
- Test writing to a connected transport (message appears on stdin)
- Test that write flushes after each message
- Test writing when stdin is already closed produces an error
- Test concurrent writes don't interleave (Mutex protects stdin)

#### REQ-T3: Process Lifecycle

**Rationale:** If process shutdown doesn't work correctly, users get zombie
processes and resource leaks. This is production-critical.

**Acceptance Criteria:**
- Test `graceful_shutdown()` with a process that exits on stdin close
- Test `graceful_shutdown()` with a process that ignores stdin close (triggers SIGTERM)
- Test `force_shutdown()` SIGTERM -> SIGKILL escalation
- Test that `close()` is idempotent (calling twice doesn't panic)
- Test that `close()` on an already-exited process works cleanly

#### REQ-C1: Control Protocol Timeout

**Rationale:** The 30-second timeout is the only protection against a hung CLI.
If it doesn't work, `request()` blocks forever.

**Acceptance Criteria:**
- Test that a request with no response times out after the configured duration
- Test that the pending request is cleaned up after timeout
- Test that the correct error type (`ClawError::ControlTimeout`) is returned
- Test that the timeout doesn't affect other pending requests

#### REQ-C2: Control Protocol Transport Write Failure

**Rationale:** If the transport write fails during `request()`, the error must
propagate correctly and the pending request must be cleaned up.

**Acceptance Criteria:**
- Test that a write failure during `request()` returns `ClawError::Connection`
- Test that the pending request entry is cleaned up on write failure
- Test that subsequent requests still work after a write failure

### 4.2 P1 -- Should Have (Important for Quality)

#### REQ-H1: HookMatcher Tests

**Rationale:** HookMatcher has literally zero tests. The `matches()` method
determines which hooks fire, making it critical for hook correctness.

**Acceptance Criteria:**
- Test `HookMatcher::all()` matches any tool name
- Test `HookMatcher::tool("Bash")` matches "Bash" and rejects "Read"
- Test `HookMatcher::tool("Bash")` is case-sensitive
- Test `HookMatcher::all()` serialization round-trip
- Test `HookMatcher::tool("X")` serialization round-trip
- Test that `matches()` with `None` tool_name returns `true` for all inputs
- Test empty string as tool name

#### REQ-H2: HookEvent Variant Coverage

**Rationale:** 8 of 10 HookEvent variants are untested for serialization.
A serde attribute error on any variant would break hook registration.

**Acceptance Criteria:**
- Test serialization/deserialization for all 10 HookEvent variants
- Test round-trip for each variant (serialize -> deserialize -> compare)
- Test that unknown variant strings produce deserialization errors

#### REQ-H3: Hook Execution Patterns

**Rationale:** Multiple hooks on the same event, hook failure handling, and
the `should_continue` flag are all untested.

**Acceptance Criteria:**
- Test that multiple hooks registered on the same event all execute
- Test that a hook returning `should_continue: false` stops processing
- Test that a hook returning an error produces `ControlResponse::Error`
- Test hook execution with various `HookInput` configurations

#### REQ-C3: Concurrent Control Requests

**Rationale:** The SDK may have multiple in-flight requests (e.g., a control
request and a hook callback simultaneously).

**Acceptance Criteria:**
- Test sending 5+ concurrent control requests and receiving correct responses
- Test that responses are routed to the correct waiting callers
- Test that cancelling one request doesn't affect others

#### REQ-M1: MCP Edge Cases

**Rationale:** The MCP server needs to handle malformed input gracefully since
it receives JSON-RPC from the CLI which could be malformed.

**Acceptance Criteria:**
- Test `tools/call` with missing `params` field
- Test `tools/call` with missing `arguments` in params (should default to `{}`)
- Test request with missing `method` field
- Test registering duplicate tool names (last-write-wins)
- Test listing tools when no tools registered (empty list)
- Test multiple servers in registry
- Test routing to nonexistent server

### 4.3 P2 -- Nice to Have (Completeness)

#### REQ-E1: Error Propagation Chains

**Acceptance Criteria:**
- Test that `ClawError::Io` wraps `std::io::Error` with correct `source()` chain
- Test that `ClawError::JsonDecode` wraps `serde_json::Error` with correct `source()`
- Test error propagation from transport write failure through `ControlProtocol::request()`
- Test error propagation from JSON decode failure through `ResponseStream`

#### REQ-E2: Error Display for All Paths

**Acceptance Criteria:**
- Test `ClawError::Process` with various exit codes and stderr contents
- Test `ClawError::MessageParse` with long raw strings (verify no truncation issues)
- Test `Debug` output for all variants

#### REQ-C4: ResponseStream Control Message Routing

**Acceptance Criteria:**
- Test that control_request messages are routed to handlers (not yielded to user)
- Test that control_response messages are routed to pending requests
- Test that malformed control messages produce `MessageParse` errors
- Test stream completion when transport channel closes

#### REQ-I1: End-to-End Integration Tests

**Acceptance Criteria:**
- Test full flow: connect transport -> initialize -> send message -> receive response -> close
- Test with mock CLI replaying different fixture types
- Test error handling when mock CLI exits unexpectedly
- Test that ResponseStream correctly parses fixture messages into typed Message enums

### 4.4 P3 -- Future Consideration

#### REQ-T4: Transport with Real-ish Subprocess

**Acceptance Criteria:**
- Enhance mock_cli to support bidirectional communication (read stdin, respond on stdout)
- Test control protocol initialize -> response handshake through real subprocess I/O
- Test that the monitor task detects unexpected process exit

#### REQ-C5: Double Initialize Protection

**Acceptance Criteria:**
- Test that calling `initialize()` twice returns an appropriate error
- Test that control requests before `initialize()` are handled correctly

---

## 5. Dependencies and Constraints

### 5.1 Test Infrastructure Dependencies

**Mock CLI (`tests/mock_cli.rs`):** The existing mock CLI replays NDJSON fixtures but
is unidirectional (stdout only). For transport I/O tests, we need to either:
1. Enhance the mock CLI to read stdin and respond accordingly, or
2. Use in-process mock transports that simulate stdin/stdout behavior

Recommendation: Use approach (2) for unit tests and approach (1) for integration tests.
The in-process approach is faster and more deterministic. The mock CLI approach validates
the real subprocess boundary.

**Tokio test runtime:** All async tests require `#[tokio::test]`. For timeout tests,
we need `tokio::time::pause()` to control time without actually waiting 30 seconds.
This requires the `tokio` `test-util` feature.

**Fixture files:** Four fixture files exist. New fixtures may be needed for:
- Control protocol messages (control_request, control_response)
- Malformed JSON responses
- Very large messages
- Rapid message bursts

### 5.2 Design Constraints

**No dependency on real Claude CLI.** All tests must work in CI without the Claude CLI
installed. Tests that conditionally run based on CLI availability (like the current
`test_validate_version_with_valid_cli`) should be marked with `#[ignore]` or gated
behind a feature flag.

**No network access.** Tests must not make API calls to Anthropic or any other service.

**Deterministic timing.** Tests that involve timeouts must use `tokio::time::pause()`
or similar techniques to avoid flaky behavior from real-time delays.

**Thread safety.** All test assertions must be deterministic even under concurrent
execution. Use `tokio::sync::Barrier`, channels, and atomics for coordination -- not
`sleep()`.

### 5.3 Risk: Refactoring Required

Some tests may require minor refactoring of the source code to improve testability:

1. **`spawn_reader_task` is a private static method.** Testing it directly requires
   either making it `pub(crate)` or testing through the `connect()` -> `messages()`
   public API path with a mock subprocess. Recommendation: Test through the public API
   using the mock CLI binary.

2. **`SubprocessCLITransport::messages()` panics on second call.** This is by design
   but makes certain test patterns awkward. Tests must carefully manage the receiver
   lifecycle.

3. **`ControlProtocol` requires `Arc<dyn Transport>`.** The existing `MockTransport`
   in `control/mod.rs` tests is a good pattern. It should be extracted to a shared
   test utility module so all modules can use it.

---

## 6. Acceptance Criteria Summary

The test initiative is complete when:

1. All P0 requirements are implemented and passing
2. All P1 requirements are implemented and passing
3. P2 requirements are implemented where practical
4. The test suite runs in under 30 seconds with `cargo test`
5. No test depends on the real Claude CLI being installed
6. No test uses `sleep()` for timing (use `tokio::time::pause()` or channels)
7. Every test has a doc comment explaining what it validates
8. The mock CLI infrastructure is used for at least 10 integration tests
9. The `MockTransport` is extracted to a shared utility and used across modules
10. `cargo test` passes on both macOS and Linux CI

---

## 7. Implementation Strategy

### 7.1 Phasing

**Phase 1 (P0 -- Foundation):** Transport I/O, process lifecycle, control timeout.
These are the highest-risk gaps and establish the test infrastructure (shared
MockTransport, enhanced mock CLI) that later phases build on.

**Phase 2 (P1 -- Breadth):** HookMatcher, HookEvent variants, concurrent operations,
MCP edge cases. These fill in the remaining high-value gaps.

**Phase 3 (P2/P3 -- Depth):** Error propagation chains, ResponseStream routing,
end-to-end integration tests. These provide defense-in-depth.

### 7.2 Shared Test Infrastructure

The following shared utilities should be created:

1. **`tests/common/mock_transport.rs`** -- Reusable `MockTransport` implementing the
   `Transport` trait, extracted from `control/mod.rs` tests. Should support:
   - Capturing all sent bytes
   - Simulating received messages
   - Configurable failure modes (write errors, connection errors)
   - Optional delay simulation

2. **`tests/common/mod.rs`** -- Common test utilities:
   - Helper to create a `ControlProtocol` with a `MockTransport`
   - Helper to create fixtures programmatically
   - Helper to assert error types

3. **Enhanced `mock_cli`** -- Add stdin reading support for bidirectional tests:
   - Read a JSON message from stdin
   - Match against a response map
   - Send the corresponding response on stdout

### 7.3 Estimated Effort

| Phase | Estimated Tests | Effort |
|-------|----------------|--------|
| Phase 1 (P0) | ~40-50 tests | High (test infrastructure + complex async tests) |
| Phase 2 (P1) | ~40-50 tests | Medium (pattern established, more straightforward) |
| Phase 3 (P2/P3) | ~20-30 tests | Low-Medium (integration assembly) |

---

## 8. Appendix: Existing Test Inventory

For reference, here is the current test distribution:

| File | Test Count | Focus |
|------|-----------|-------|
| `transport/subprocess.rs` | 7 | Construction, not-connected errors |
| `transport/discovery.rs` | 7 | CLI path search, version validation |
| `control/mod.rs` | 7 | Request/response with MockTransport |
| `control/messages.rs` | 13 | Serialization round-trips |
| `control/handlers.rs` | 7 | Handler trait implementations, registry |
| `control/pending.rs` | 7 | Insert/complete/cancel, concurrent access |
| `mcp_server.rs` | 24 | Tool registration, JSON-RPC routing |
| `hooks/types.rs` | 7 | HookInput, HookContext construction |
| `hooks/callback.rs` | 4 | HookCallback trait, struct and closure impls |
| `hooks/response.rs` | 7 | HookResponse construction, serialization |
| `error.rs` | 11 | Error message formatting, From conversions |
| `options.rs` | ~14 | Builder pattern, CLI args generation |
| `client.rs` | ~14 | Client construction, not-connected errors |
| `tests/integration_test.rs` | ~25 | Mock CLI, message parsing, agent definitions |
| **Total** | **~116** | |

<!-- END OF DOCUMENT -->

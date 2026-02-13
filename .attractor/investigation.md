# Investigation: rusty_claw-sna - Implement query() function

**Task ID:** rusty_claw-sna
**Priority:** P1
**Status:** IN_PROGRESS
**Date:** 2026-02-13

## Task Summary

Implement the public `query()` function that accepts a prompt and optional options, spawns a transport, streams NDJSON messages, and returns `impl Stream<Item = Result<Message, ClawError>>`.

## Current State

### ✅ Completed Dependencies
1. **rusty_claw-6cn** - Transport trait and SubprocessCLITransport implemented
   - `Transport` trait defined in `crates/rusty_claw/src/transport/mod.rs`
   - `SubprocessCLITransport` fully implemented with async I/O
   - `CliDiscovery` integrated for automatic CLI discovery and version validation

2. **rusty_claw-pwc** - Shared types and message structs implemented
   - `Message` enum with System, Assistant, User, Result variants
   - `ContentBlock` enum for text, tool use, tool result, thinking
   - Complete message parsing with serde support
   - All types in `crates/rusty_claw/src/messages.rs`

3. **rusty_claw-k71** - CLI discovery and version check implemented
   - `CliDiscovery::find()` searches PATH and common locations
   - `CliDiscovery::validate_version()` ensures CLI >= 2.0.0
   - Integrated into `SubprocessCLITransport::connect()`

### ❌ Missing Components

1. **No query() function** - The main public API is not implemented
2. **No ClaudeAgentOptions** - Configuration builder doesn't exist yet (blocked task rusty_claw-dss)
3. **No stream adapter** - Need to convert `mpsc::UnboundedReceiver<Result<Value, ClawError>>` to `impl Stream<Item = Result<Message, ClawError>>`

## Required Changes

### 1. Create `crates/rusty_claw/src/query.rs` (NEW FILE)

This file will contain the implementation of the `query()` function. Based on the spec and existing code:

```rust
//! Simple query API for one-shot Claude interactions
//!
//! The `query()` function provides a convenient way to send a prompt to Claude
//! and receive a stream of response messages.

use tokio_stream::{Stream, StreamExt};
use crate::error::ClawError;
use crate::messages::Message;
use crate::transport::{SubprocessCLITransport, Transport};

/// Execute a one-shot query to Claude and return a stream of messages
///
/// This function:
/// 1. Creates a SubprocessCLITransport (discovers CLI automatically)
/// 2. Connects to the CLI process
/// 3. Sends the prompt to the CLI
/// 4. Returns a stream of parsed Message structs
///
/// # Arguments
///
/// * `prompt` - The prompt string to send to Claude
/// * `options` - Optional configuration (for now, accepts None since ClaudeAgentOptions not yet implemented)
///
/// # Returns
///
/// A stream of `Result<Message, ClawError>` that yields messages until the CLI closes
///
/// # Errors
///
/// - `ClawError::CliNotFound` if Claude CLI is not found
/// - `ClawError::InvalidCliVersion` if CLI version < 2.0.0
/// - `ClawError::Connection` if transport fails to connect
/// - `ClawError::JsonDecode` if message parsing fails
///
/// # Example
///
/// ```ignore
/// use rusty_claw::query;
/// use tokio_stream::StreamExt;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let mut stream = query("What files are in this directory?", None).await?;
///
///     while let Some(result) = stream.next().await {
///         match result {
///             Ok(msg) => println!("{:?}", msg),
///             Err(e) => eprintln!("Error: {}", e),
///         }
///     }
///     Ok(())
/// }
/// ```
pub async fn query(
    prompt: impl Into<String>,
    options: Option<()>, // TODO: Change to ClaudeAgentOptions when rusty_claw-dss is complete
) -> Result<impl Stream<Item = Result<Message, ClawError>>, ClawError> {
    let prompt = prompt.into();

    // TODO: Extract CLI args from options when ClaudeAgentOptions exists
    let args = vec![
        "--output-format=stream-json".to_string(),
        "--verbose".to_string(),
        "-p".to_string(),
        prompt,
    ];

    // Create transport with auto-discovery (None = discover CLI)
    let mut transport = SubprocessCLITransport::new(None, args);

    // Connect to CLI (discovers, validates version, spawns process)
    transport.connect().await?;

    // Get the message receiver from transport
    let rx = transport.messages();

    // Convert receiver to stream and parse Message structs
    let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx)
        .map(|result| {
            result.and_then(|value| {
                serde_json::from_value::<Message>(value).map_err(|e| {
                    ClawError::MessageParse {
                        reason: e.to_string(),
                        raw: format!("{:?}", e),
                    }
                })
            })
        });

    Ok(stream)
}
```

**Key Design Decisions:**
- **Placeholder for options**: Accept `Option<()>` now, will change to `Option<ClaudeAgentOptions>` in future task
- **Auto-discovery**: Pass `None` to SubprocessCLITransport for automatic CLI discovery
- **Stream conversion**: Use `tokio_stream::wrappers::UnboundedReceiverStream` to convert mpsc receiver
- **Message parsing**: Parse `serde_json::Value` into typed `Message` structs
- **Error handling**: Convert JSON errors to `ClawError::MessageParse`

### 2. Modify `crates/rusty_claw/src/lib.rs`

Add the query module and re-export the function:

```rust
// Add after existing modules
pub mod query;

// Add to prelude
pub mod prelude {
    //! Common imports for rusty_claw users
    //!
    //! Use `use rusty_claw::prelude::*;` to import commonly used types.

    pub use crate::error::ClawError;
    pub use crate::messages::{
        ApiMessage, AssistantMessage, ContentBlock, McpServerInfo, Message, ResultMessage,
        StreamEvent, SystemMessage, ToolInfo, UsageInfo, UserMessage,
    };
    pub use crate::transport::{CliDiscovery, SubprocessCLITransport, Transport};
    pub use crate::query::query; // NEW
}
```

**Also add top-level re-export for convenience:**
```rust
// Public API re-exports
pub use query::query;
```

## Implementation Strategy

### Step 1: Create query.rs with basic implementation (15 min)
- Write function signature matching spec
- Implement transport creation and connection
- Convert receiver to stream
- Add message parsing with error handling

### Step 2: Update lib.rs (5 min)
- Add `pub mod query;`
- Re-export `query` function at top level
- Add to prelude

### Step 3: Write tests (20 min)
- Test with valid prompt (will require mock CLI or skip for now)
- Test error cases (CLI not found, version mismatch)
- Test streaming behavior
- Test message parsing

### Step 4: Update documentation (5 min)
- Add module-level docs
- Add function-level docs with examples
- Update lib.rs overview

## Success Criteria

- ✅ `query()` function signature matches spec
- ✅ Transport integration working (auto-discovery, version check)
- ✅ Message streaming functional
- ✅ Stream returns `impl Stream<Item = Result<Message, ClawError>>`
- ✅ Error handling complete (CLI not found, connection, parsing)
- ✅ Function exported from lib.rs and prelude
- ✅ All unit tests passing
- ✅ Zero clippy warnings
- ✅ Documentation complete with examples

## Test Requirements

### Unit Tests
- ✅ Test query with empty prompt (should work)
- ✅ Test query with None options (should work)
- ✅ Test error handling for CLI not found (use invalid cli_path)
- ✅ Test error handling for invalid version
- ✅ Test message parsing (valid and invalid JSON)

### Integration Tests (Deferred)
- ⏸️ Test with real CLI (requires installation)
- ⏸️ Test with mock CLI subprocess
- ⏸️ End-to-end test with actual query and response

**Note:** Full integration tests will be added in rusty_claw-isy (P2 task: Add integration tests with mock CLI)

## Risks & Mitigation

### Risk 1: ClaudeAgentOptions Not Yet Implemented (MEDIUM)
- **Impact:** Can't pass configuration options to query()
- **Mitigation:** Use `Option<()>` placeholder, hardcode default args
- **Future:** Update signature when rusty_claw-dss is complete
- **Breaking Change:** Yes, but acceptable at 0.1.0

### Risk 2: No Mock CLI for Testing (MEDIUM)
- **Impact:** Can't write full integration tests yet
- **Mitigation:** Write unit tests for parsing and error handling
- **Mitigation:** Integration tests deferred to rusty_claw-isy
- **Future:** Full test coverage when mock CLI exists

### Risk 3: Stream Lifetime and Ownership (LOW)
- **Impact:** Transport must live as long as stream
- **Mitigation:** Transport is owned by the stream (via closure or Arc)
- **Current:** Transport dropped early - need to fix
- **Solution:** Keep transport alive in stream (use Arc or move into task)

### Risk 4: CLI Args Construction (LOW)
- **Impact:** Hardcoded args may not work for all cases
- **Mitigation:** Use minimal set of required args (--output-format, --verbose, -p)
- **Future:** Build args from ClaudeAgentOptions when available

## Files to Create/Modify

### New Files
1. `crates/rusty_claw/src/query.rs` - Main query() implementation (150 lines)

### Modified Files
1. `crates/rusty_claw/src/lib.rs` - Add query module, re-exports (10 lines)

### Test Files (Optional for this task)
1. `crates/rusty_claw/src/query.rs` - Unit tests in #[cfg(test)] module (50 lines)

## Dependencies

**Runtime Dependencies:**
- ✅ tokio (async runtime)
- ✅ tokio-stream (stream utilities)
- ✅ serde_json (message parsing)
- ✅ All dependencies already in Cargo.toml

**Type Dependencies:**
- ✅ `Message` enum from messages.rs
- ✅ `ClawError` enum from error.rs
- ✅ `Transport` trait and `SubprocessCLITransport` from transport/
- ❌ `ClaudeAgentOptions` (blocked, will use placeholder)

## Next Steps After Implementation

1. **rusty_claw-qrl** (P2) - Implement ClaudeClient for interactive sessions
   - Will use query() as foundation
   - Adds stateful, multi-turn interactions

2. **rusty_claw-dss** (P2) - Implement ClaudeAgentOptions builder
   - Update query() signature to accept `Option<ClaudeAgentOptions>`
   - Pass options to transport for CLI arg construction

3. **rusty_claw-isy** (P2) - Add integration tests with mock CLI
   - Full end-to-end tests for query()
   - Mock CLI subprocess for testing

## Estimated Effort

- **Implementation:** 15 minutes (query.rs)
- **Integration:** 5 minutes (lib.rs updates)
- **Unit Tests:** 20 minutes (basic tests)
- **Documentation:** 5 minutes (docstrings)
- **Total:** ~45 minutes

## Notes

- The query() function is the simplest entry point to the SDK
- It's designed for one-shot queries (fire and forget)
- More complex use cases (multi-turn, hooks, permissions) will use ClaudeClient
- The stream is consumed by the caller, transport cleanup happens automatically on drop
- Transport lifetime management is critical - transport must outlive the stream

## Transport Lifetime Issue (CRITICAL)

⚠️ **Important:** The current implementation has a **transport lifetime bug**:

```rust
// ❌ WRONG - transport dropped before stream is consumed
pub async fn query(...) -> Result<impl Stream<...>, ClawError> {
    let mut transport = SubprocessCLITransport::new(...);
    transport.connect().await?;
    let rx = transport.messages();
    let stream = UnboundedReceiverStream::new(rx).map(...);
    Ok(stream) // transport dropped here, process killed!
}
```

**Solution:** Keep transport alive alongside stream:

```rust
// ✅ CORRECT - transport lives as long as stream
pub async fn query(...) -> Result<impl Stream<...>, ClawError> {
    let mut transport = SubprocessCLITransport::new(...);
    transport.connect().await?;
    let rx = transport.messages();

    let stream = UnboundedReceiverStream::new(rx)
        .map(parse_message);

    // Wrap in a struct that owns both transport and stream
    Ok(QueryStream::new(transport, stream))
}
```

**Alternative:** Spawn background task that owns transport:

```rust
pub async fn query(...) -> Result<impl Stream<...>, ClawError> {
    let mut transport = SubprocessCLITransport::new(...);
    transport.connect().await?;
    let rx = transport.messages();

    // Spawn task that owns transport and keeps it alive
    tokio::spawn(async move {
        // Transport is moved here, will live until task ends
        let _transport = transport;
        // Stream receiver will close when transport drops
    });

    let stream = UnboundedReceiverStream::new(rx).map(...);
    Ok(stream)
}
```

**Recommendation:** Use the wrapper struct approach for cleaner lifecycle management.

## Investigation Complete

**Status:** ✅ Ready for implementation
**Confidence:** High (all dependencies exist, clear path forward)
**Blockers:** None
**Next Step:** Create query.rs and implement function

# Current Task: rusty_claw-sna

**Task ID:** rusty_claw-sna
**Status:** IN_PROGRESS
**Priority:** P1
**Type:** Task
**Owner:** Scott Nixon

## Title
Implement query() function

## Description
Implement the public query() function that accepts a prompt and options, spawns a transport, streams NDJSON messages, and returns impl Stream<Item = Result<Message, ClawError>>.

## Dependencies (All Completed ✅)
- ✅ rusty_claw-6cn: Implement Transport trait and SubprocessCLITransport (P1)
- ✅ rusty_claw-pwc: Define shared types and message structs (P1)
- ✅ rusty_claw-k71: Implement CLI discovery and version check (P2)

## Blocks
- ○ rusty_claw-qrl: Implement ClaudeClient for interactive sessions (P2)

## Key Requirements

1. **Function Signature:**
   ```rust
   pub async fn query(
       prompt: impl Into<String>,
       options: Option<ClaudeAgentOptions>,
   ) -> impl Stream<Item = Result<Message, ClawError>>
   ```

2. **Core Functionality:**
   - Accept a prompt string and optional ClaudeAgentOptions
   - Automatically discover and launch the Claude CLI
   - Spawn a SubprocessCLITransport connection
   - Send the query message to the transport
   - Stream NDJSON responses as Message structs
   - Handle errors gracefully

3. **Message Flow:**
   - Create Transport connection via SubprocessCLITransport::connect()
   - Send Control::Query message with prompt and options
   - Read NDJSON stream of responses
   - Parse each line as a Message
   - Yield results to the stream
   - Handle transport errors and convert to ClawError

4. **Error Handling:**
   - Return ClawError for CLI not found, version mismatch, transport errors, parsing errors, etc.

5. **Return Type:**
   - Use Rust streams (async generator or futures::Stream)
   - Return `impl Stream<Item = Result<Message, ClawError>>`
   - Support cancellation via drop

## Files to Create/Modify
- Modify: `crates/rusty_claw/src/lib.rs` - Add query() function
- Potentially create: `crates/rusty_claw/src/client.rs` - Query implementation

## Test Coverage Needed
- Test with valid prompt
- Test with ClaudeAgentOptions
- Test error cases (CLI not found, invalid version, etc.)
- Test streaming behavior
- Test message parsing

## Success Criteria
- ✅ Function signature matches spec
- ✅ Transport integration working
- ✅ Message streaming functional
- ✅ Error handling complete
- ✅ All unit tests passing
- ✅ Zero clippy warnings
- ✅ Documentation complete

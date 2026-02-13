# Current Task: rusty_claw-6cn

**Status:** IN_PROGRESS
**Priority:** P1 (Critical)
**Type:** Task
**Owner:** Scott Nixon

## Title
Implement Transport trait and SubprocessCLITransport

## Description
Define async Transport trait (connect, write, messages, end_input, close, is_ready). Implement SubprocessCLITransport that spawns claude CLI with correct args, reads NDJSON from stdout, and handles process lifecycle (SIGTERM/SIGKILL).

## Dependencies
- ✅ Depends on: rusty_claw-9pf (Define error hierarchy) - COMPLETED

## Blocks
- ○ rusty_claw-91n (Implement Control Protocol handler) - P1
- ○ rusty_claw-sna (Implement query() function) - P1

## Key Requirements
1. Define async Transport trait with methods:
   - connect() → establish subprocess connection
   - write() → write to stdin
   - messages() → async stream of NDJSON messages from stdout
   - end_input() → close stdin gracefully
   - close() → terminate subprocess
   - is_ready() → check if transport is connected

2. Implement SubprocessCLITransport:
   - Spawn "claude" CLI subprocess with correct args
   - Read NDJSON messages from stdout in real-time
   - Handle process lifecycle (SIGTERM/SIGKILL)
   - Proper error handling and cleanup

3. Message Handling:
   - Parse NDJSON output from Claude CLI
   - Convert to Message types (from messages.rs)
   - Stream messages as they arrive

## Files to Create/Modify
- Create: `crates/rusty_claw/src/transport.rs` - Transport trait and SubprocessCLITransport impl
- Modify: `crates/rusty_claw/src/lib.rs` - Export transport module

## Reference
- SPEC.md: Transport trait specification
- docs/ARCHITECTURE.md: Transport layer design
- Message types: `crates/rusty_claw/src/messages.rs`
- Error hierarchy: `crates/rusty_claw/src/error.rs`

## Test Coverage Needed
- Transport trait interface tests
- SubprocessCLITransport spawning tests
- NDJSON parsing tests
- Process lifecycle management tests
- Error handling tests

## Success Criteria
- ✅ Transport trait fully defined with all required methods
- ✅ SubprocessCLITransport implementation complete
- ✅ Subprocess spawning and lifecycle management working
- ✅ NDJSON message parsing from stdout
- ✅ All unit tests passing
- ✅ Zero clippy warnings in new code
- ✅ Documentation complete

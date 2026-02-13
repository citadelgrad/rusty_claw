# Current Task: rusty_claw-1ke

## Task Details
- **ID:** rusty_claw-1ke
- **Title:** Add unit tests for message parsing and fixtures
- **Type:** task
- **Priority:** P2 (Medium)
- **Status:** in_progress
- **Owner:** Scott Nixon

## Description
Create test fixtures (simple_query.ndjson, tool_use.ndjson, error_response.ndjson, etc.) and unit tests to verify deserialization of all Message variants and ContentBlock types.

## Dependencies
- ✅ **rusty_claw-pwc:** Define shared types and message structs (COMPLETED)

## Blocks
- ⏳ **rusty_claw-isy:** Add integration tests with mock CLI (blocked by this task)

## Acceptance Criteria

1. **Create Test Fixtures:**
   - `crates/rusty_claw/tests/fixtures/simple_query.ndjson` - Simple query/response exchange
   - `crates/rusty_claw/tests/fixtures/tool_use.ndjson` - Tool use request and result
   - `crates/rusty_claw/tests/fixtures/error_response.ndjson` - Error response handling
   - Additional variants as needed for comprehensive coverage

2. **Implement Unit Tests:**
   - Test deserialization of each Message variant
   - Test deserialization of each ContentBlock type
   - Verify error handling for malformed JSON
   - Test edge cases (empty strings, null values, etc.)

3. **Test Execution:**
   - `cargo test --lib message` should pass all tests
   - Zero clippy warnings
   - Good test coverage of all variants

4. **Documentation:**
   - Document fixtures and their purpose
   - Add examples for using fixtures in tests

## Key Files
- `crates/rusty_claw/src/message.rs` - Message types and tests
- `crates/rusty_claw/tests/fixtures/` - Test fixtures directory (to be created)

## Implementation Notes
- Fixtures should be representative of real Claude API responses
- Tests should verify correct deserialization with valid data
- Error cases tested separately in error module
- All Message variants need coverage: Text, ToolUse, ToolResult
- All ContentBlock types need coverage

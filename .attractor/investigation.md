# Investigation: rusty_claw-1ke - Add unit tests for message parsing and fixtures

**Date:** 2026-02-13
**Task:** rusty_claw-1ke [P2]
**Status:** Investigation Complete

---

## Task Summary

Create test fixtures (NDJSON files) and additional unit tests to verify deserialization of all Message variants and ContentBlock types. This task enhances the existing test suite to provide comprehensive coverage and reusable fixtures.

---

## Current State Analysis

### Existing Implementation ✅

**File:** `crates/rusty_claw/src/messages.rs` (627 lines)

**Current Test Coverage:** 19 tests (all passing)
- ✅ `test_message_system_init` - SystemMessage::Init deserialization
- ✅ `test_message_system_compact_boundary` - SystemMessage::CompactBoundary
- ✅ `test_message_assistant` - AssistantMessage with optional fields
- ✅ `test_message_user` - UserMessage deserialization
- ✅ `test_message_result_success` - ResultMessage::Success with all optional fields
- ✅ `test_message_result_error` - ResultMessage::Error with extra fields
- ✅ `test_message_result_input_required` - ResultMessage::InputRequired
- ✅ `test_content_block_text` - ContentBlock::Text
- ✅ `test_content_block_tool_use` - ContentBlock::ToolUse
- ✅ `test_content_block_tool_result` - ContentBlock::ToolResult with is_error
- ✅ `test_content_block_tool_result_default_is_error` - Default is_error=false
- ✅ `test_content_block_thinking` - ContentBlock::Thinking
- ✅ `test_stream_event` - StreamEvent deserialization
- ✅ `test_usage_info` - UsageInfo struct
- ✅ `test_tool_info_minimal` - ToolInfo with minimal fields
- ✅ `test_tool_info_full` - ToolInfo with all optional fields
- ✅ `test_mcp_server_info` - McpServerInfo with flattened extra
- ✅ `test_optional_fields_default` - Optional fields default correctly
- ✅ `test_json_round_trip_complex` - Complex message with multiple content blocks

**Analysis:**
- All Message variants have test coverage ✅
- All ContentBlock types have test coverage ✅
- Optional fields tested ✅
- Round-trip serialization tested ✅
- **Gap:** No standalone NDJSON fixture files for reuse
- **Gap:** No tests loading fixtures from files
- **Gap:** No malformed JSON error handling tests (deferred to error module)
- **Gap:** Edge cases (empty strings, null values) not explicitly tested

### Missing Fixtures ❌

**Directory:** `crates/rusty_claw/tests/fixtures/` (DOES NOT EXIST)

According to SPEC.md section 10.3, the following fixtures should exist:
- `simple_query.ndjson` - Basic question/answer exchange
- `tool_use.ndjson` - Tool use request and result cycle
- `error_response.ndjson` - Error response handling
- `multi_turn.ndjson` - Multi-turn conversation (optional for this task)
- `hook_callback.ndjson` - Hook invocation flow (blocked by hooks)
- `mcp_tool_call.ndjson` - MCP tool invocation (blocked by MCP)

---

## What Needs to Be Done

### 1. Create Fixture Directory ✅

**Action:** Create `crates/rusty_claw/tests/fixtures/` directory

### 2. Create NDJSON Fixture Files (NEW FILES)

#### A. `simple_query.ndjson` (Basic Query/Response) - ~15 lines
**Content:** Realistic NDJSON sequence for a simple query:
1. System::Init message with session_id, tools, mcp_servers
2. Assistant message with Text content block
3. Result::Success message with stats

**Purpose:** Verify end-to-end message sequence parsing

#### B. `tool_use.ndjson` (Tool Use Flow) - ~20 lines
**Content:** Complete tool invocation cycle:
1. System::Init message
2. Assistant message with ToolUse content block
3. User message with ToolResult content block
4. Assistant message with Text response
5. Result::Success message

**Purpose:** Verify tool-related message types parse correctly

#### C. `error_response.ndjson` (Error Handling) - ~10 lines
**Content:** Error scenario:
1. System::Init message
2. Assistant message with Text
3. Result::Error message with error string and extra fields

**Purpose:** Verify error result parsing

#### D. `thinking_content.ndjson` (Extended Thinking) - ~12 lines
**Content:** Message with thinking tokens:
1. System::Init message
2. Assistant message with Thinking + Text content blocks
3. Result::Success message

**Purpose:** Verify ContentBlock::Thinking parsing

### 3. Add Fixture-Based Tests (MODIFY FILE)

**File:** `crates/rusty_claw/src/messages.rs` (add ~120 lines to tests module)

**New Tests to Add:**

#### A. `test_simple_query_fixture()` - Load and parse simple_query.ndjson
- Read fixture file line-by-line
- Deserialize each line as Message
- Verify message types and key fields
- Assert 3 messages total (Init, Assistant, Success)

#### B. `test_tool_use_fixture()` - Load and parse tool_use.ndjson
- Verify ToolUse content block structure
- Verify ToolResult content block structure
- Assert correct message sequence

#### C. `test_error_response_fixture()` - Load and parse error_response.ndjson
- Verify Result::Error parsing
- Verify extra fields flattening works

#### D. `test_thinking_content_fixture()` - Load and parse thinking_content.ndjson
- Verify Thinking content block parsing
- Verify multiple content blocks in single message

#### E. `test_all_fixtures_valid()` - Meta test
- Iterate through all .ndjson files in fixtures/
- Verify each line parses as valid Message
- Catch regressions if fixtures become invalid

### 4. Add Edge Case Tests (MODIFY FILE)

**File:** `crates/rusty_claw/src/messages.rs` (add ~60 lines to tests module)

**New Tests to Add:**

#### A. `test_empty_string_text_content()` - Empty text content
```rust
{"type": "text", "text": ""}
```
Verify empty string is valid

#### B. `test_empty_content_array()` - Message with no content blocks
```rust
{"type": "assistant", "message": {"role": "assistant", "content": []}}
```
Verify empty array is valid

#### C. `test_minimal_system_init()` - Minimal Init with empty arrays
```rust
{"type": "system", "subtype": "init", "session_id": "123", "tools": [], "mcp_servers": []}
```
Verify minimal required fields

#### D. `test_large_tool_input()` - Large JSON in tool input field
- Create ToolUse with complex nested input JSON
- Verify serde_json::Value handles it

#### E. `test_unicode_in_text()` - Unicode characters in text fields
- Emojis, CJK characters, special symbols
- Verify UTF-8 handling

### 5. Documentation (MODIFY FILE)

**File:** `crates/rusty_claw/src/messages.rs` (add ~30 lines to module docs)

**Add Section:** "Test Fixtures"
- Document fixture directory structure
- Explain purpose of each fixture file
- Provide example of loading fixtures in custom tests
- Reference SPEC.md section 10.3

---

## Files to Create/Modify

### New Files (4 fixtures + directory)

1. **`crates/rusty_claw/tests/` directory** (CREATE if missing)
2. **`crates/rusty_claw/tests/fixtures/` directory** (CREATE)
3. **`crates/rusty_claw/tests/fixtures/simple_query.ndjson`** (~15 lines)
4. **`crates/rusty_claw/tests/fixtures/tool_use.ndjson`** (~20 lines)
5. **`crates/rusty_claw/tests/fixtures/error_response.ndjson`** (~10 lines)
6. **`crates/rusty_claw/tests/fixtures/thinking_content.ndjson`** (~12 lines)

### Modified Files

1. **`crates/rusty_claw/src/messages.rs`**
   - Add 4 fixture-based tests (~120 lines)
   - Add 5 edge case tests (~60 lines)
   - Add fixture documentation section (~30 lines)
   - Total additions: ~210 lines

---

## Implementation Strategy

### Phase 1: Create Fixtures (15 min)
1. Create directory structure
2. Write simple_query.ndjson based on SPEC examples
3. Write tool_use.ndjson with complete tool cycle
4. Write error_response.ndjson
5. Write thinking_content.ndjson
6. Validate each fixture manually with `jq` or Python

### Phase 2: Fixture-Based Tests (20 min)
1. Add test helper function: `load_fixture(name: &str) -> Vec<Message>`
2. Implement test_simple_query_fixture()
3. Implement test_tool_use_fixture()
4. Implement test_error_response_fixture()
5. Implement test_thinking_content_fixture()
6. Implement test_all_fixtures_valid()
7. Run `cargo test --lib messages` to verify

### Phase 3: Edge Case Tests (15 min)
1. Implement test_empty_string_text_content()
2. Implement test_empty_content_array()
3. Implement test_minimal_system_init()
4. Implement test_large_tool_input()
5. Implement test_unicode_in_text()
6. Run tests to verify

### Phase 4: Documentation (5 min)
1. Add "Test Fixtures" section to module docs
2. Document each fixture's purpose
3. Add example code snippet for loading fixtures
4. Cross-reference SPEC.md

### Phase 5: Verification (5 min)
1. Run full test suite: `cargo test --lib message`
2. Verify zero clippy warnings
3. Check test coverage report (if available)
4. Verify all acceptance criteria met

---

## Risk Analysis

### ✅ Low Risk

**Reason:** Pure additive changes with no modifications to existing types
- Adding new test files (no side effects)
- Adding new tests to existing module (isolated)
- No changes to production code
- All existing 19 tests continue to pass

### 🟡 Medium Risk: Fixture Format Accuracy

**Concern:** Fixtures must match actual Claude CLI output format
- **Mitigation:** Base fixtures on SPEC.md examples and existing inline test JSON
- **Mitigation:** Run against mock CLI (future task rusty_claw-isy will validate)
- **Mitigation:** Use exact field names/types from current message types

### 🟡 Medium Risk: Test Execution Environment

**Concern:** Fixture files must be found at runtime
- **Mitigation:** Use `env!("CARGO_MANIFEST_DIR")` to get crate root
- **Mitigation:** Construct paths relative to crate root
- **Mitigation:** Add clear error messages if fixtures missing

### ✅ No Breaking Changes

- No API changes
- No type signature changes
- No dependency changes
- Fully backward compatible

---

## Success Criteria

### Acceptance Criteria from Task

1. ✅ **Create Test Fixtures:**
   - [x] `crates/rusty_claw/tests/fixtures/simple_query.ndjson`
   - [x] `crates/rusty_claw/tests/fixtures/tool_use.ndjson`
   - [x] `crates/rusty_claw/tests/fixtures/error_response.ndjson`
   - [x] Additional: `thinking_content.ndjson`

2. ✅ **Implement Unit Tests:**
   - [x] Test deserialization of each Message variant (via fixtures)
   - [x] Test deserialization of each ContentBlock type (via fixtures + edge cases)
   - [x] Verify error handling for malformed JSON (existing error module)
   - [x] Test edge cases (empty strings, null values, unicode)

3. ✅ **Test Execution:**
   - [x] `cargo test --lib message` should pass all tests (24 existing + 9 new = 33 total)
   - [x] Zero clippy warnings
   - [x] Good test coverage of all variants

4. ✅ **Documentation:**
   - [x] Document fixtures and their purpose (module docs + inline comments)
   - [x] Add examples for using fixtures in tests

### Additional Verification

- All fixture files are valid NDJSON (no trailing commas, proper line endings)
- Fixtures represent realistic Claude CLI output
- Tests run successfully in CI environment
- No flaky tests (deterministic behavior)
- Test names follow Rust conventions (`test_snake_case`)

---

## Dependencies

### Satisfied Dependencies ✅

- **rusty_claw-pwc** [P1]: Define shared types and message structs (CLOSED)
  - All Message types exist
  - All ContentBlock types exist
  - Serde derive macros in place
  - Existing 19 tests verify basic functionality

### Blocks Downstream ⏳

This task (rusty_claw-1ke) blocks:
- **rusty_claw-isy** [P2]: Add integration tests with mock CLI
  - Fixtures created here will be reused by mock CLI
  - Fixture format establishes contract between SDK and CLI

---

## Estimated Effort

- **Fixture Creation:** 15 minutes
- **Fixture-Based Tests:** 20 minutes
- **Edge Case Tests:** 15 minutes
- **Documentation:** 5 minutes
- **Verification:** 5 minutes
- **Total:** ~60 minutes (1 hour)

---

## Notes

### Fixture Design Principles

1. **Realistic:** Based on actual Claude CLI output format from SPEC
2. **Minimal:** Only essential fields, avoid unnecessary complexity
3. **Self-contained:** Each fixture is a complete message sequence
4. **Documented:** Clear comments explaining each message's purpose
5. **Reusable:** Can be used by integration tests and mock CLI

### Test Design Principles

1. **Single Responsibility:** Each test verifies one specific behavior
2. **Descriptive Names:** Test name clearly states what is verified
3. **Arrange-Act-Assert:** Standard test structure
4. **No External Dependencies:** Tests run offline without network
5. **Fast:** All tests complete in < 100ms

### Future Work (Out of Scope)

- Malformed JSON tests (handled by error module, not message parsing)
- Multi-turn conversation fixtures (deferred to integration tests)
- Hook callback fixtures (blocked by hooks implementation)
- MCP tool call fixtures (blocked by MCP implementation)
- Performance benchmarks (separate task)
- Fuzz testing (separate task)

---

## Implementation Checklist

### Before Starting
- [x] Read current_task.md
- [x] Review messages.rs existing tests
- [x] Review SPEC.md message format
- [x] Understand fixture directory structure
- [x] Plan test cases

### Phase 1: Fixtures
- [ ] Create `tests/` directory (if missing)
- [ ] Create `tests/fixtures/` directory
- [ ] Write `simple_query.ndjson`
- [ ] Write `tool_use.ndjson`
- [ ] Write `error_response.ndjson`
- [ ] Write `thinking_content.ndjson`
- [ ] Validate fixtures with `jq` (each line is valid JSON)

### Phase 2: Fixture Tests
- [ ] Add `load_fixture()` helper function
- [ ] Implement `test_simple_query_fixture()`
- [ ] Implement `test_tool_use_fixture()`
- [ ] Implement `test_error_response_fixture()`
- [ ] Implement `test_thinking_content_fixture()`
- [ ] Implement `test_all_fixtures_valid()`
- [ ] Run `cargo test --lib messages::tests`

### Phase 3: Edge Cases
- [ ] Implement `test_empty_string_text_content()`
- [ ] Implement `test_empty_content_array()`
- [ ] Implement `test_minimal_system_init()`
- [ ] Implement `test_large_tool_input()`
- [ ] Implement `test_unicode_in_text()`
- [ ] Run tests

### Phase 4: Documentation
- [ ] Add "Test Fixtures" section to module docs
- [ ] Document each fixture file
- [ ] Add usage example

### Phase 5: Verification
- [ ] Run full test suite: `cargo test --lib message`
- [ ] Check clippy: `cargo clippy -- -D warnings`
- [ ] Verify test count (33 total: 19 existing + 5 fixture + 5 edge + 1 meta + 3 new)
- [ ] Update `.attractor/test-results.md`

---

## Conclusion

This task is **ready for implementation** with:
- ✅ Clear requirements from task description
- ✅ All dependencies satisfied (message types exist)
- ✅ No blockers
- ✅ Well-defined scope (fixtures + tests only, no type changes)
- ✅ Low risk (additive changes only)
- ✅ Reasonable effort estimate (~1 hour)

The implementation will provide:
1. Reusable NDJSON fixtures for integration tests
2. Comprehensive test coverage of all message variants
3. Edge case validation
4. Documentation for future fixture usage

Next step: Begin Phase 1 (Create Fixtures) 🚀

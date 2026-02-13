# Investigation: rusty_claw-b4s - Implement Subagent Support

**Task ID:** rusty_claw-b4s
**Priority:** P3
**Status:** IN_PROGRESS
**Date:** 2026-02-13

---

## Executive Summary

This task requires completing the subagent support infrastructure that is already partially implemented in the codebase. The `AgentDefinition` struct exists but needs to be properly integrated into the control protocol's initialize request, and the `SubagentStart`/`SubagentStop` hook events are already defined but need examples and documentation.

**Current State:** ✅ Foundation exists (75% complete)
- `AgentDefinition` struct is defined in `options.rs` with all required fields
- `HookEvent` enum already includes `SubagentStart` and `SubagentStop` variants
- `Initialize` control request already includes `agents` field
- Builder pattern already supports `.agents()` method

**What's Missing:** 🔨 Integration and documentation (25% remaining)
1. Ensure `AgentDefinition` is properly serialized in initialize control request
2. Add comprehensive tests for agent registration
3. Create example showing subagent usage
4. Document the subagent lifecycle and hook events

---

## Current Implementation Analysis

### 1. AgentDefinition Struct (options.rs:202-213)

**Status:** ✅ COMPLETE

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDefinition {
    /// Agent description
    pub description: String,
    /// Agent prompt
    pub prompt: String,
    /// Allowed tools
    pub tools: Vec<String>,
    /// Model override
    pub model: Option<String>,
}
```

**Analysis:**
- Matches SPEC.md section 5.1 exactly
- Already has `Serialize`/`Deserialize` derives (required for control protocol)
- All fields match Python SDK structure
- Public API is correct

**Verification:**
```rust
// From SPEC.md:
// pub struct AgentDefinition {
//     pub description: String,
//     pub prompt: String,
//     pub tools: Vec<String>,
//     pub model: Option<String>,
// }
```
✅ **No changes needed**

---

### 2. HookEvent Enum (options.rs:128-151)

**Status:** ✅ COMPLETE

```rust
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum HookEvent {
    // ... other events ...
    SubagentStop,    // Line 142
    SubagentStart,   // Line 144
    // ... other events ...
}
```

**Analysis:**
- Both `SubagentStart` and `SubagentStop` variants exist
- Properly annotated with `#[serde(rename_all = "PascalCase")]`
- Will serialize as `"SubagentStart"` and `"SubagentStop"` in JSON

**Verification:**
```rust
// From SPEC.md section 6.1:
// pub enum HookEvent {
//     SubagentStop,
//     SubagentStart,
// }
```
✅ **No changes needed**

---

### 3. ClaudeAgentOptions (options.rs:234-303)

**Status:** ✅ COMPLETE

```rust
pub struct ClaudeAgentOptions {
    // ... other fields ...

    // Subagents (placeholder for future tasks)
    /// Agent definitions
    pub agents: HashMap<String, AgentDefinition>,  // Line 268

    // ... other fields ...
}
```

**Analysis:**
- `agents` field exists with correct type
- Uses `HashMap<String, AgentDefinition>` (agent name → definition)
- Already included in builder pattern

**Verification:**
```rust
// From SPEC.md section 5.1:
// pub agents: HashMap<String, AgentDefinition>
```
✅ **No changes needed**

---

### 4. ClaudeAgentOptionsBuilder (options.rs:440-594)

**Status:** ✅ COMPLETE

```rust
impl ClaudeAgentOptionsBuilder {
    // ... other methods ...

    /// Set agents
    pub fn agents(mut self, agents: HashMap<String, AgentDefinition>) -> Self {
        self.inner.agents = agents;  // Line 513-516
        self
    }

    // ... other methods ...
}
```

**Analysis:**
- Builder method exists with correct signature
- Follows same pattern as other builder methods

**Verification:**
✅ **No changes needed**

---

### 5. Initialize Control Request (control/messages.rs:56-95)

**Status:** ✅ COMPLETE

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum ControlRequest {
    Initialize {
        #[serde(skip_serializing_if = "HashMap::is_empty", default)]
        hooks: HashMap<HookEvent, Vec<HookMatcher>>,

        /// Agent definitions for spawning subagents
        #[serde(skip_serializing_if = "HashMap::is_empty", default)]
        agents: HashMap<String, AgentDefinition>,  // Line 82-83

        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        sdk_mcp_servers: Vec<SdkMcpServer>,

        #[serde(skip_serializing_if = "Option::is_none")]
        permissions: Option<PermissionMode>,

        can_use_tool: bool,
    },
    // ... other variants ...
}
```

**Analysis:**
- `agents` field is properly included in `Initialize` variant
- Has correct serde attributes:
  - `#[serde(skip_serializing_if = "HashMap::is_empty", default)]` - omits empty maps
- Type is correct: `HashMap<String, AgentDefinition>`
- Documentation comment exists

**Verification:**
```rust
// From SPEC.md section 4.3:
// Initialize {
//     hooks: HashMap<HookEvent, Vec<HookMatcher>>,
//     agents: HashMap<String, AgentDefinition>,  // ✅
//     sdk_mcp_servers: Vec<SdkMcpServer>,
//     permissions: Option<PermissionMode>,
//     can_use_tool: bool,
// }
```
✅ **No changes needed**

---

## What Needs to Be Done

### Task 1: Add Integration Test for Agent Registration

**File:** `crates/rusty_claw/tests/integration/agent_test.rs` (NEW FILE)

**Purpose:** Verify that agents are properly serialized in the initialize control request

**Test Cases:**
1. ✅ `test_agent_definition_serialization` - Verify AgentDefinition serializes correctly
2. ✅ `test_initialize_with_agents` - Verify Initialize request includes agents field
3. ✅ `test_agent_registration_empty` - Verify empty agents map is omitted from JSON
4. ✅ `test_agent_registration_multiple` - Verify multiple agents serialize correctly

**Estimated Lines:** ~150 lines

---

### Task 2: Create Subagent Example

**File:** `examples/subagent_usage.rs` (NEW FILE)

**Purpose:** Demonstrate how to define and use subagents with hooks

**Estimated Lines:** ~60 lines

---

### Task 3: Add Subagent Hook Documentation

**File:** `docs/HOOKS.md` (NEW SECTION)

**Purpose:** Document SubagentStart and SubagentStop hook lifecycle

**Estimated Lines:** ~80 lines

---

### Task 4: Update README/Main Documentation

**File:** `README.md` or `docs/README.md`

**Purpose:** Add subagent section to main documentation

**Estimated Lines:** ~30 lines

---

## Files to Create/Modify

### New Files (3 files, ~290 lines total)

| File | Lines | Purpose |
|------|-------|---------|
| `crates/rusty_claw/tests/integration/agent_test.rs` | ~150 | Integration tests for agent registration |
| `examples/subagent_usage.rs` | ~60 | Example showing subagent usage |
| `docs/HOOKS.md` (new section) | ~80 | Document SubagentStart/SubagentStop hooks |

### Modified Files (1 file, ~30 lines added)

| File | Changes | Purpose |
|------|---------|---------|
| `README.md` or `docs/README.md` | Add subagent section | User-facing documentation |

### No Changes Required (5 files, 0 lines)

| File | Reason |
|------|--------|
| `crates/rusty_claw/src/options.rs` | AgentDefinition already complete |
| `crates/rusty_claw/src/control/messages.rs` | Initialize already includes agents |

---

## Dependencies & Prerequisites

### ✅ Satisfied Dependencies

1. **rusty_claw-qrl** (Implement ClaudeClient) - CLOSED ✓
   - ClaudeClient exists and is functional
   - Control protocol is implemented
   - Initialize request is working

### 🔗 No Blockers

All required infrastructure exists:
- ✅ `AgentDefinition` struct is complete
- ✅ `HookEvent::SubagentStart` and `HookEvent::SubagentStop` exist
- ✅ `Initialize` control request includes `agents` field
- ✅ Builder pattern supports `.agents()` method
- ✅ Serialization/deserialization works (derives present)

---

## Testing Strategy

### Integration Tests (Required)

**File:** `crates/rusty_claw/tests/integration/agent_test.rs`

**Test Coverage:**
1. ✅ AgentDefinition serialization
2. ✅ Initialize request with agents
3. ✅ Empty agents map handling (should be omitted from JSON)
4. ✅ Multiple agents in single request
5. ✅ Optional model field (Some vs None)

**Execution:**
```bash
cargo test --test agent_test
```

---

## Implementation Phases

### Phase 1: Integration Tests (30 min)

**Goal:** Verify agent registration works correctly

**Tasks:**
1. Create `tests/integration/agent_test.rs`
2. Write 4 test cases (serialization, registration, empty, multiple)
3. Run tests with `cargo test --test agent_test`
4. Verify all tests pass

**Success Criteria:**
- ✅ All 4 tests pass
- ✅ AgentDefinition serializes to correct JSON
- ✅ Initialize request includes agents field
- ✅ Empty agents map is omitted from JSON

---

### Phase 2: Example Code (20 min)

**Goal:** Demonstrate subagent usage

**Tasks:**
1. Create `examples/subagent_usage.rs`
2. Define 2 example agents (researcher, writer)
3. Show hook registration for SubagentStart/SubagentStop
4. Add comprehensive documentation comments

**Success Criteria:**
- ✅ Example compiles without errors
- ✅ Code is well-documented
- ✅ Shows both agent definition and hook usage

---

### Phase 3: Hook Documentation (25 min)

**Goal:** Document SubagentStart/SubagentStop hooks

**Tasks:**
1. Create or update `docs/HOOKS.md`
2. Add SubagentStart hook documentation
3. Add SubagentStop hook documentation
4. Include JSON examples and code samples

**Success Criteria:**
- ✅ Hook input format documented
- ✅ Use cases listed
- ✅ Code examples provided

---

### Phase 4: Main Documentation (15 min)

**Goal:** Add subagent section to README

**Tasks:**
1. Update `README.md` or `docs/README.md`
2. Add subagent section with code example
3. Link to example file and hooks documentation

**Success Criteria:**
- ✅ Subagent section added to main docs
- ✅ Links to examples and detailed docs
- ✅ Clear, concise explanation

---

### Phase 5: Verification (10 min)

**Goal:** Ensure everything works together

**Tasks:**
1. Run all tests: `cargo test`
2. Build examples: `cargo build --examples`
3. Check documentation: `cargo doc --open`
4. Verify no clippy warnings: `cargo clippy`

**Success Criteria:**
- ✅ All tests pass
- ✅ All examples compile
- ✅ Documentation builds
- ✅ Zero clippy warnings

---

## Acceptance Criteria

From task description: "Implement AgentDefinition struct and subagent configuration in options. Support SubagentStart/SubagentStop hook events and agent registration in the initialize control request."

### Requirement 1: AgentDefinition struct ✅ COMPLETE

**Status:** ✅ Already implemented in `options.rs:202-213`

### Requirement 2: Subagent configuration in options ✅ COMPLETE

**Status:** ✅ Already implemented in `options.rs:268` and builder at `options.rs:513-516`

### Requirement 3: SubagentStart/SubagentStop hook events ✅ COMPLETE

**Status:** ✅ Already implemented in `options.rs:142-144`

### Requirement 4: Agent registration in initialize control request ✅ COMPLETE

**Status:** ✅ Already implemented in `control/messages.rs:82-83`

---

## What Actually Needs Implementation

### Core Implementation: ✅ 100% COMPLETE

All required code already exists:
- ✅ AgentDefinition struct
- ✅ HookEvent variants
- ✅ Options field and builder
- ✅ Initialize request field

### Testing & Documentation: 🔨 0% COMPLETE (Required Work)

What needs to be added:
1. Integration tests (~150 lines)
2. Example code (~60 lines)
3. Hook documentation (~80 lines)
4. README section (~30 lines)

**Total New Code:** ~320 lines across 3-4 files

---

## Risk Assessment

### Low Risk

**Why:**
- All core infrastructure already exists and is tested
- No changes to existing code required
- Only adding tests and documentation
- No breaking changes
- No complex logic to implement

### Success Probability

**95% - Very High**

**Reasoning:**
1. Core implementation is complete (verified by code inspection)
2. Only need to add tests and examples (straightforward)
3. No dependencies on external tasks
4. Clear specification in SPEC.md
5. Similar patterns already exist in codebase

---

## Time Estimate

| Phase | Task | Duration |
|-------|------|----------|
| 1 | Write integration tests | 30 min |
| 2 | Create example code | 20 min |
| 3 | Write hook documentation | 25 min |
| 4 | Update main documentation | 15 min |
| 5 | Verification and testing | 10 min |
| **Total** | | **100 min (1.7 hours)** |

---

## Summary

**Status:** ✅ Ready to implement (all blockers cleared)

**Complexity:** 🟢 LOW - Core implementation already complete

**Scope:** Testing + Documentation only (~320 new lines)

**Confidence:** 95% - High confidence of success

**Estimated Time:** 1.7 hours

**Key Finding:** The task description says "Implement AgentDefinition struct and subagent configuration" but this work is already complete! The actual work needed is:
- Add comprehensive integration tests
- Create usage examples
- Document the hook events
- Update user-facing documentation

This is a documentation and testing task, not an implementation task.

---

**Investigation Status:** ✅ COMPLETE
**Ready to Proceed:** YES
**Blockers:** NONE
**Next Action:** Phase 1 - Write integration tests

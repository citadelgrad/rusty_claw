# Investigation: rusty_claw-s8q - Implement Permission Management

## Task Overview

**Task ID:** rusty_claw-s8q
**Title:** Implement permission management
**Priority:** P2 (High)
**Type:** task

Implement a comprehensive permission management system for controlling tool usage in Claude agents. This builds on top of the Hook system (rusty_claw-bip) to provide flexible, policy-based permission control.

## What Exists (Foundation)

### 1. Hook System (Completed in rusty_claw-bip)
**Location:** `crates/rusty_claw/src/hooks/`

- ✅ **HookEvent enum** (options.rs:100-121) - 10 lifecycle events
- ✅ **HookMatcher** (options.rs:140-170) - Pattern matching for hook triggering
- ✅ **HookCallback trait** (hooks/callback.rs) - Trait for implementing hook logic
- ✅ **HookResponse** (hooks/response.rs:54-147) - Response with permission decisions
- ✅ **PermissionDecision enum** (hooks/response.rs:18-25) - Allow/Deny/Ask decisions
- ✅ **HookInput/HookContext** (hooks/types.rs) - Data structures for hook invocation

### 2. Control Protocol Handler
**Location:** `crates/rusty_claw/src/control/`

- ✅ **CanUseToolHandler trait** (handlers.rs:78-97) - Trait for tool permission checks
- ✅ **ControlHandlers registry** (handlers.rs:220-344) - Handler registration system
- ✅ **IncomingControlRequest::CanUseTool** (messages.rs:236-242) - Message type for tool permission requests
- ✅ **handle_incoming()** (mod.rs:389-end) - Routes can_use_tool requests to handlers

**Current Default Behavior:**
```rust
// If no handler registered, allow all tools (line 389-399)
if let Some(handler) = &handlers.can_use_tool {
    match handler.can_use_tool(&tool_name, &tool_input).await {
        Ok(allowed) => ControlResponse::Success {
            data: json!({ "allowed": allowed }),
        },
        Err(e) => ControlResponse::Error {
            error: e.to_string(),
            extra: json!({}),
        },
    }
} else {
    // Default: allow all tools
    ControlResponse::Success {
        data: json!({ "allowed": true }),
    }
}
```

### 3. Options Configuration
**Location:** `crates/rusty_claw/src/options.rs`

- ✅ **PermissionMode enum** (lines 48-72) - Currently has 4 modes:
  - Default
  - AcceptEdits
  - BypassPermissions
  - Plan
- ✅ **permission_prompt_tool_allowlist** (line 224) - Vec<String> field in ClaudeAgentOptions
- ✅ **allowed_tools** (line 218) - Vec<String> field for explicit tool allowlist
- ✅ **disallowed_tools** (line 220) - Vec<String> field for explicit tool denylist

## What's Missing (Implementation Needed)

### 1. Enhanced PermissionMode Enum ❌
**Location:** `src/options.rs` (needs modification)

**Current Implementation:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    Default,
    AcceptEdits,
    BypassPermissions,
    Plan,
}
```

**Required Enhancement:**
- Add `Ask` variant for prompt-on-use behavior
- Add `Deny` variant for blocking all tools
- Add `Custom` variant for hook-based permission logic
- Keep existing variants for backward compatibility

**Acceptance Criteria:** PermissionMode enum with Ask/Deny/Custom variants

### 2. Permission Policy Logic ❌
**Location:** NEW FILE - `src/permissions/mod.rs` (create new module)

**Required Components:**
- Policy evaluation engine that:
  - Checks tool against allowlist/denylist
  - Routes permission checks through Hook system
  - Falls back to PermissionMode default policy
  - Returns PermissionDecision (Allow/Deny/Ask)

**Acceptance Criteria:** Permission policy implementation with hook integration

### 3. DefaultPermissionHandler Implementation ❌
**Location:** NEW FILE - `src/permissions/handler.rs`

**Required Implementation:**
```rust
pub struct DefaultPermissionHandler {
    mode: PermissionMode,
    allowed_tools: Vec<String>,
    disallowed_tools: Vec<String>,
    // ... hook registry for custom callbacks
}

impl CanUseToolHandler for DefaultPermissionHandler {
    async fn can_use_tool(&self, tool_name: &str, tool_input: &Value) -> Result<bool, ClawError> {
        // 1. Check disallowed_tools first (explicit deny)
        // 2. Check allowed_tools (explicit allow)
        // 3. Invoke hooks for custom logic
        // 4. Fall back to PermissionMode default policy
    }
}
```

**Acceptance Criteria:** DefaultPermissionHandler with policy evaluation

### 4. Hook Integration for Permission Events ❌
**Location:** Modify `src/control/mod.rs` handle_incoming()

**Current Behavior:**
- Can_use_tool requests go directly to CanUseToolHandler
- No hook invocation for permission checks

**Required Enhancement:**
- Before invoking CanUseToolHandler, check for registered hooks
- If hooks registered for PreToolUse event:
  - Construct HookInput with tool_name and tool_input
  - Invoke hook callbacks
  - Process HookResponse.permission_decision
  - Return early if hook denies or asks
- If no hooks or hook allows, proceed to CanUseToolHandler

**Acceptance Criteria:** Hook integration for can_use_tool events

### 5. Permission Configuration Builder ❌
**Location:** Modify `src/options.rs` builder

**Required Enhancement:**
- Add builder methods for permission configuration:
  - `.permission_mode(PermissionMode)`
  - `.allowed_tools(Vec<String>)`
  - `.disallowed_tools(Vec<String>)`
  - `.permission_prompt_tool_allowlist(Vec<String>)`

**Note:** Builder methods already exist (lines 451-462), but may need updates for new PermissionMode variants

**Acceptance Criteria:** Builder pattern for permission configuration

## Files to Create/Modify

### New Files (2 files, ~400 lines)

1. **`src/permissions/mod.rs`** (~150 lines)
   - Module documentation with permission system overview
   - Re-exports for public types
   - Permission policy evaluation logic
   - Integration with Hook system

2. **`src/permissions/handler.rs`** (~250 lines)
   - DefaultPermissionHandler struct
   - CanUseToolHandler trait implementation
   - Tool allowlist/denylist checking
   - Hook callback invocation
   - Default policy fallback
   - Comprehensive unit tests (~100 lines)

### Modified Files (5 files)

3. **`src/options.rs`** (+20 lines)
   - Update PermissionMode enum with Ask/Deny/Custom variants
   - Update serialization to match CLI expectations
   - Update to_cli_arg() method for new variants
   - Add unit tests for new variants

4. **`src/control/mod.rs`** (+50 lines)
   - Modify handle_incoming() to invoke hooks before CanUseToolHandler
   - Add HookInput construction for can_use_tool events
   - Process HookResponse.permission_decision
   - Update integration tests

5. **`src/lib.rs`** (+5 lines)
   - Add `pub mod permissions;` declaration
   - Update prelude to export permission types

6. **`src/hooks/response.rs`** (+30 lines)
   - Add helper methods for permission-specific responses
   - Add PermissionDecision conversion helpers
   - Add unit tests for new methods

7. **`Cargo.toml`** (no changes needed)
   - All required dependencies already present

### Test Files (integrated into module files)

8. **`src/permissions/handler.rs` tests** (~100 lines)
   - Test allowlist/denylist logic
   - Test hook integration
   - Test default policy fallback
   - Test PermissionMode variants
   - Test edge cases (empty lists, wildcards TODO)

## Architecture

### Permission Check Flow

```
CLI sends can_use_tool request
          ↓
ControlProtocol.handle_incoming()
          ↓
Check for PreToolUse hooks
          ↓
     [Hooks registered?]
          ↓
    YES         NO
     ↓           ↓
Invoke hooks → CanUseToolHandler.can_use_tool()
     ↓               ↓
HookResponse → DefaultPermissionHandler
     ↓               ↓
[permission_decision?]  [Check policy]
     ↓               ↓
 Allow/Deny/Ask  Allow/Deny/Ask
     ↓               ↓
Send control_response to CLI
```

### Policy Evaluation Order (in DefaultPermissionHandler)

1. **Explicit Deny** - Check disallowed_tools first (highest priority)
2. **Explicit Allow** - Check allowed_tools second
3. **Hook Decision** - Invoke registered hooks for custom logic
4. **Default Policy** - Fall back to PermissionMode:
   - `Allow` → Allow all tools
   - `Deny` → Deny all tools
   - `Ask` → Ask user for each tool
   - `Custom` → Require hook (error if no hook)
   - `Default/AcceptEdits/BypassPermissions/Plan` → Use CLI defaults

### Hook Integration Points

**PreToolUse Event:**
- Triggered before tool execution
- HookInput contains tool_name and tool_input
- HookResponse.permission_decision determines outcome
- If hook denies, tool execution is blocked
- If hook allows, proceed to CanUseToolHandler
- If no hooks, proceed to CanUseToolHandler

**Hook Priority:**
- Hooks are checked BEFORE CanUseToolHandler
- Multiple hooks can be registered (evaluated in order)
- First hook that returns Deny or Ask wins
- If all hooks Allow, proceed to handler

## Implementation Plan

### Phase 1: Update PermissionMode Enum (30 min)
- Add Ask/Deny/Custom variants to PermissionMode
- Update serialization and to_cli_arg()
- Add unit tests
- **Files:** options.rs

### Phase 2: Create Permissions Module (60 min)
- Create src/permissions/ directory
- Create mod.rs with module docs
- Define DefaultPermissionHandler struct
- Implement basic structure (no logic yet)
- **Files:** permissions/mod.rs, permissions/handler.rs

### Phase 3: Implement Policy Logic (90 min)
- Implement can_use_tool() in DefaultPermissionHandler
- Add allowlist/denylist checking
- Add default policy fallback
- Add unit tests for policy logic
- **Files:** permissions/handler.rs

### Phase 4: Hook Integration (60 min)
- Modify handle_incoming() to invoke hooks
- Construct HookInput for can_use_tool events
- Process HookResponse.permission_decision
- Update integration tests
- **Files:** control/mod.rs

### Phase 5: Response Helpers (30 min)
- Add permission-specific helper methods to HookResponse
- Add PermissionDecision conversion helpers
- Add unit tests
- **Files:** hooks/response.rs

### Phase 6: Module Integration (30 min)
- Update lib.rs with permissions module
- Update prelude to export types
- Update Cargo.toml if needed (likely not)
- **Files:** lib.rs

### Phase 7: Testing (90 min)
- Write comprehensive unit tests (~15-20 tests)
- Write integration tests with Hook system
- Test all PermissionMode variants
- Test allowlist/denylist combinations
- Test hook priority and decision flow
- Verify no regressions in existing tests
- **Files:** permissions/handler.rs, control/mod.rs

### Phase 8: Documentation & Verification (30 min)
- Write module-level docs with examples
- Write rustdoc for all public types
- Run full test suite (cargo test)
- Run clippy (cargo clippy)
- Verify acceptance criteria
- Update .attractor/ docs
- **Files:** permissions/mod.rs, permissions/handler.rs

**Total Estimated Time:** ~6.5 hours

## Success Criteria

### Acceptance Criteria (from task description)

1. ✅ **PermissionMode enum** - Define permission check policies (Allow/Ask/Deny/Custom)
2. ✅ **Tool allowlist** - Selective permission prompting with allowlist/denylist
3. ✅ **can_use_tool callback handler** - Route permission checks through handlers
4. ✅ **Hook integration** - Permission decisions through hooks with fallback
5. ✅ **Default permission policy** - Fallback behavior based on PermissionMode
6. ✅ **Comprehensive tests** - ~15-20 unit tests + integration tests with Hook system
7. ✅ **Complete documentation** - Module-level docs with examples

### Quality Standards

- ✅ Zero compilation errors
- ✅ Zero clippy warnings in new code
- ✅ Zero test failures
- ✅ Zero regressions in existing tests (126 unit + 44 doc tests)
- ✅ 100% documentation coverage of public API
- ✅ All doctests pass

## Dependencies

**Completed Tasks (All Satisfied):**
- ✅ rusty_claw-bip: Implement Hook system [P2] - **CLOSED**

**Blocks Downstream Tasks:**
- ○ rusty_claw-isy: Add integration tests [P2]

## Risks & Considerations

### 1. Breaking Changes
**Risk:** Adding new PermissionMode variants might break existing code
**Mitigation:** Keep existing variants unchanged, add new ones, update serialization carefully

### 2. Hook Priority Logic
**Risk:** Complex hook evaluation order could cause unexpected behavior
**Mitigation:** Document hook priority clearly, add comprehensive tests, follow "first deny wins" rule

### 3. Default Policy Confusion
**Risk:** Multiple permission sources (allowlist, hooks, mode) could confuse users
**Mitigation:** Clear documentation of evaluation order, simple priority rules, examples for each case

### 4. Performance
**Risk:** Hook invocation on every tool use could add latency
**Mitigation:** Async trait already in place, hooks are optional, allowlist checked first

### 5. Backward Compatibility
**Risk:** Existing CanUseToolHandler implementations might break
**Mitigation:** DefaultPermissionHandler is opt-in, existing handlers still work, no API changes to trait

## Notes

- **Python SDK Reference:** Check claude-agent-sdk-python for permission patterns
- **Hook System:** Already complete with PermissionDecision (Allow/Deny/Ask) ✅
- **Control Protocol:** Already routes can_use_tool to handlers ✅
- **Test Coverage:** Existing 126 unit + 44 doc tests must continue passing ✅
- **Clippy:** Target zero warnings in new code ✅

## Open Questions

None - all architecture decisions clear from task description and existing code.

---

**Status:** Ready for implementation (Phase 1)
**Next Step:** Update PermissionMode enum with Ask/Deny/Custom variants

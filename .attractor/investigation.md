# Investigation: rusty_claw-bip - Implement Hook system

## Task Summary

**ID:** rusty_claw-bip
**Title:** Implement Hook system
**Priority:** P2 (High)
**Status:** in_progress
**Owner:** Scott Nixon

**Description:** Implement HookEvent enum, HookMatcher, HookCallback trait with blanket impl for closures, HookResponse with permission decisions, and hook invocation routing from control protocol callbacks.

**Dependencies (All Completed ✓):**
- ✓ rusty_claw-91n: Implement Control Protocol handler [P1]

**Blocks (Downstream Tasks):**
- ○ rusty_claw-s8q: Implement permission management [P2]

## Current State Analysis

### What Exists ✅

**1. Control Protocol (✓ Complete - from rusty_claw-91n)**
- `src/control/mod.rs` - ControlProtocol with request/response routing
- `src/control/messages.rs` - Control message types with serde
- `src/control/handlers.rs` - Handler traits (CanUseToolHandler, HookHandler, McpMessageHandler)
- `src/control/pending.rs` - Pending request tracking
- Full integration with Control Protocol including `IncomingControlRequest::HookCallback`

**2. Control Handlers (✓ Partial)**
- `HookHandler` trait exists in `src/control/handlers.rs`:
  ```rust
  #[async_trait]
  pub trait HookHandler: Send + Sync {
      async fn call(&self, hook_event: HookEvent, hook_input: Value) -> Result<Value, ClawError>;
  }
  ```
- Handler registry in `ControlHandlers` with HashMap<String, Arc<dyn HookHandler>>
- Registration method: `register_hook(hook_id: String, handler: Arc<dyn HookHandler>)`

**3. Placeholder Types (✓ Exist but minimal)**
- `src/options.rs`:
  - `HookEvent` - Currently a placeholder empty struct
  - `HookMatcher` - Currently a placeholder empty struct
- These are referenced in `control/messages.rs` for `Initialize` and `HookCallback` requests

**4. Control Message Integration (✓ Complete)**
- `IncomingControlRequest::HookCallback` variant exists with:
  - `hook_id: String`
  - `hook_event: HookEvent`
  - `hook_input: Value`
- `ControlRequest::Initialize` accepts `hooks: HashMap<HookEvent, Vec<HookMatcher>>`

### What's Missing ❌

**Critical Implementation Gaps:**

1. **HookEvent enum** (REPLACE in `src/options.rs`, ~50 lines)
   - Currently: Empty placeholder struct
   - Needed: Full enum with variants (PreToolUse, PostToolUse, etc.)
   - Per SPEC.md Section 6.1:
     ```rust
     #[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
     #[serde(rename_all = "PascalCase")]
     pub enum HookEvent {
         PreToolUse,
         PostToolUse,
         PostToolUseFailure,
         UserPromptSubmit,
         Stop,
         SubagentStop,
         SubagentStart,
         PreCompact,
         Notification,
         PermissionRequest,
     }
     ```

2. **HookMatcher struct** (REPLACE in `src/options.rs`, ~40 lines)
   - Currently: Empty placeholder struct
   - Needed: Pattern matching for tool names
   - Per SPEC.md Section 6.2:
     ```rust
     #[derive(Debug, Clone, Serialize, Deserialize)]
     pub struct HookMatcher {
         /// Tool name pattern to match (e.g., "Bash", "mcp__*")
         #[serde(skip_serializing_if = "Option::is_none")]
         pub tool_name: Option<String>,
     }
     ```
   - Should support:
     - Exact match: "Bash"
     - Wildcard: "mcp__*"
     - None (match all tools)

3. **HookCallback trait with blanket impl** (NEW FILE `src/hooks/callback.rs`, ~80 lines)
   - Trait definition with async call method
   - Blanket impl for closures: `impl<F, Fut> HookCallback for F where F: Fn(...) -> Fut`
   - Per SPEC.md Section 6.2:
     ```rust
     #[async_trait]
     pub trait HookCallback: Send + Sync {
         async fn call(
             &self,
             input: HookInput,
             tool_use_id: Option<&str>,
             context: &HookContext,
         ) -> Result<HookResponse, ClawError>;
     }
     ```

4. **HookInput and HookContext** (NEW FILE `src/hooks/types.rs`, ~60 lines)
   - HookInput: Data passed to hooks (tool name, input, etc.)
   - HookContext: Session context (tools, agents, etc.)
   - Needs to be designed based on hook event types

5. **HookResponse struct** (NEW FILE `src/hooks/response.rs`, ~80 lines)
   - Permission decisions (Allow/Deny/Ask)
   - Additional context injection
   - Tool input modification
   - Per SPEC.md Section 6.3:
     ```rust
     #[derive(Debug, Clone, Default, Serialize)]
     pub struct HookResponse {
         #[serde(skip_serializing_if = "Option::is_none")]
         pub permission_decision: Option<PermissionDecision>,

         #[serde(skip_serializing_if = "Option::is_none")]
         pub permission_decision_reason: Option<String>,

         #[serde(skip_serializing_if = "Option::is_none")]
         pub additional_context: Option<String>,

         #[serde(rename = "continue", default = "default_true")]
         pub should_continue: bool,

         #[serde(skip_serializing_if = "Option::is_none")]
         pub updated_input: Option<serde_json::Value>,
     }

     #[derive(Debug, Clone, Serialize)]
     #[serde(rename_all = "lowercase")]
     pub enum PermissionDecision {
         Allow,
         Deny,
         Ask,
     }
     ```

6. **Hook invocation routing** (MODIFY `src/control/mod.rs`, +50 lines)
   - Integrate hook dispatch with control protocol
   - Route `IncomingControlRequest::HookCallback` to registered hooks
   - Return `HookResponse` as `ControlResponse`
   - Currently `HookHandler` trait exists but hook matching/routing logic is missing

7. **Hooks module structure** (NEW FILE `src/hooks/mod.rs`, ~30 lines)
   - Module organization and re-exports
   - Public API surface
   - Documentation with examples

## Design Analysis

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         Hooks Module                            │
│                                                                 │
│  ┌───────────────────────────────────────────────────────────┐ │
│  │                  HookEvent Enum                           │ │
│  │  PreToolUse, PostToolUse, PostToolUseFailure,            │ │
│  │  UserPromptSubmit, Stop, SubagentStop, etc.              │ │
│  └───────────────────────────────────────────────────────────┘ │
│                             ↓                                   │
│  ┌───────────────────────────────────────────────────────────┐ │
│  │                  HookMatcher                              │ │
│  │  Pattern matching: tool_name (exact, wildcard, None)     │ │
│  └───────────────────────────────────────────────────────────┘ │
│                             ↓                                   │
│  ┌───────────────────────────────────────────────────────────┐ │
│  │                  HookCallback Trait                       │ │
│  │  async fn call(input, tool_use_id, context) -> Response  │ │
│  │  + Blanket impl for Fn closures                          │ │
│  └───────────────────────────────────────────────────────────┘ │
│                             ↓                                   │
│  ┌───────────────────────────────────────────────────────────┐ │
│  │                  HookResponse                             │ │
│  │  Permission decision (Allow/Deny/Ask)                    │ │
│  │  Additional context, updated input, continue flag        │ │
│  └───────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
                             ↕
┌─────────────────────────────────────────────────────────────────┐
│                  Control Protocol Handler                       │
│  IncomingControlRequest::HookCallback                          │
│  → Dispatch to HookHandler via registry                        │
│  → Return HookResponse as ControlResponse                      │
└─────────────────────────────────────────────────────────────────┘
```

### Key Design Patterns

**1. Hook Event Types (SPEC.md 6.1)**
- Enum with PascalCase serialization
- Hash + Eq + PartialEq for HashMap keys
- Comprehensive set of lifecycle events:
  - Tool lifecycle: PreToolUse, PostToolUse, PostToolUseFailure
  - User interaction: UserPromptSubmit
  - Session control: Stop, SubagentStop, SubagentStart
  - System events: PreCompact, Notification, PermissionRequest

**2. Hook Matching (SPEC.md 6.2)**
- Flexible pattern matching:
  - `tool_name: None` → Match all tools
  - `tool_name: Some("Bash")` → Exact match
  - `tool_name: Some("mcp__*")` → Wildcard (future enhancement)
- Multiple matchers per event in `HashMap<HookEvent, Vec<HookMatcher>>`

**3. Closure Support (SPEC.md 13.1)**
- Blanket trait impl for ergonomics:
  ```rust
  impl<F, Fut> HookCallback for F
  where
      F: Fn(HookInput, Option<&str>, &HookContext) -> Fut + Send + Sync,
      Fut: Future<Output = Result<HookResponse, ClawError>> + Send,
  {
      async fn call(&self, input: HookInput, tool_use_id: Option<&str>, context: &HookContext)
          -> Result<HookResponse, ClawError>
      {
          self(input, tool_use_id, context).await
      }
  }
  ```
- Allows both trait objects and closures as callbacks

**4. Permission Decisions (SPEC.md 6.3)**
- Three-way decision: Allow, Deny, Ask
- Optional reason for user-facing messages
- Additional context injection for Claude
- Tool input transformation
- Continue flag for hook chaining

**5. Integration with Control Protocol**
- `HookHandler` trait already exists in control/handlers.rs
- Need to:
  1. Update `HookHandler::call` signature to match new types
  2. Implement hook matching in control protocol handler
  3. Convert `HookResponse` to `ControlResponse::Success`

### Implementation Strategy

**Phase 1: Core Types (90 min)**
1. Replace `HookEvent` placeholder with full enum (~20 min)
2. Replace `HookMatcher` placeholder with full struct (~20 min)
3. Implement `HookInput` and `HookContext` (~30 min)
4. Implement `HookResponse` and `PermissionDecision` (~20 min)

**Phase 2: Callback Trait (60 min)**
1. Define `HookCallback` trait in hooks module (~15 min)
2. Implement blanket impl for closures (~30 min)
3. Update `HookHandler` in control/handlers.rs (~15 min)

**Phase 3: Hook Matching (45 min)**
1. Implement matcher logic (exact match, wildcard) (~30 min)
2. Unit tests for pattern matching (~15 min)

**Phase 4: Control Integration (60 min)**
1. Update control protocol hook dispatch (~30 min)
2. Convert HookResponse to ControlResponse (~15 min)
3. Integration tests (~15 min)

**Phase 5: Module Structure (30 min)**
1. Create hooks/mod.rs with re-exports (~10 min)
2. Update lib.rs prelude (~5 min)
3. Module-level documentation (~15 min)

**Phase 6: Documentation (45 min)**
1. Doctests for all public types (~20 min)
2. Usage examples in module docs (~15 min)
3. Architecture documentation (~10 min)

**Phase 7: Testing (90 min)**
1. Unit tests for all types (~30 min)
2. Integration tests with control protocol (~30 min)
3. Closure blanket impl tests (~15 min)
4. Pattern matching tests (~15 min)

**Phase 8: Verification (30 min)**
1. Run full test suite (~10 min)
2. Clippy checks (~5 min)
3. Documentation coverage (~5 min)
4. Acceptance criteria checklist (~10 min)

**Total Estimated Time: 7.5 hours**

## Files to Create/Modify

### New Files (6 files, ~350 lines)

1. **`src/hooks/mod.rs`** (~50 lines)
   - Module structure and re-exports
   - Public API documentation
   - Examples

2. **`src/hooks/callback.rs`** (~80 lines)
   - `HookCallback` trait definition
   - Blanket impl for closures
   - Documentation with examples

3. **`src/hooks/types.rs`** (~70 lines)
   - `HookInput` struct
   - `HookContext` struct
   - Helper methods

4. **`src/hooks/response.rs`** (~80 lines)
   - `HookResponse` struct
   - `PermissionDecision` enum
   - Builder pattern helpers

5. **`src/hooks/matcher.rs`** (~70 lines)
   - Pattern matching logic
   - Wildcard support
   - Unit tests

### Modified Files (4 files)

6. **`src/options.rs`** (REPLACE ~50 lines)
   - Replace `HookEvent` placeholder → full enum
   - Replace `HookMatcher` placeholder → move to hooks module
   - Import from hooks module

7. **`src/control/handlers.rs`** (MODIFY +15 lines)
   - Update `HookHandler::call` signature to use new types
   - Update tests to use new types

8. **`src/control/mod.rs`** (MODIFY +50 lines)
   - Implement hook matching in `handle_incoming`
   - Route to appropriate hook based on event + matcher
   - Convert HookResponse to ControlResponse

9. **`src/lib.rs`** (MODIFY +10 lines)
   - Replace empty `hooks` module with real implementation
   - Update prelude with hook types

## Required Changes - Detailed

### 1. Replace HookEvent in options.rs

**Current (line 88-90):**
```rust
/// Hook event type (placeholder for future hook tasks)
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct HookEvent;
```

**New:**
```rust
/// Hook event type - triggers for lifecycle callbacks
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    UserPromptSubmit,
    Stop,
    SubagentStop,
    SubagentStart,
    PreCompact,
    Notification,
    PermissionRequest,
}
```

### 2. Replace HookMatcher in options.rs

**Current (line 92-94):**
```rust
/// Hook matcher (placeholder for future hook tasks)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookMatcher;
```

**New:**
```rust
/// Hook matcher for pattern-based hook triggering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookMatcher {
    /// Tool name pattern to match (e.g., "Bash", "mcp__*", or None for all)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

impl HookMatcher {
    /// Create a matcher that matches all tools
    pub fn all() -> Self {
        Self { tool_name: None }
    }

    /// Create a matcher for a specific tool name
    pub fn tool(name: impl Into<String>) -> Self {
        Self { tool_name: Some(name.into()) }
    }

    /// Check if this matcher matches the given tool name
    pub fn matches(&self, tool_name: &str) -> bool {
        match &self.tool_name {
            None => true,  // Match all
            Some(pattern) => {
                // TODO: Add wildcard support (mcp__*)
                pattern == tool_name
            }
        }
    }
}
```

### 3. Update HookHandler in control/handlers.rs

**Current signature (line 129-141):**
```rust
#[async_trait]
pub trait HookHandler: Send + Sync {
    async fn call(&self, hook_event: HookEvent, hook_input: Value) -> Result<Value, ClawError>;
}
```

**Keep as-is** - This signature is fine for control protocol integration. The new `HookCallback` trait in the hooks module will have the richer signature, and we'll bridge between them.

### 4. Add hook invocation in control/mod.rs

**Location:** In `handle_incoming` method (needs to be created/updated)

**Add:**
```rust
async fn dispatch_hook(&self, hook_id: &str, hook_event: HookEvent, hook_input: Value)
    -> Result<ControlResponse, ClawError>
{
    let handlers = self.handlers.lock().await;

    if let Some(handler) = handlers.hook_callbacks.get(hook_id) {
        let result = handler.call(hook_event, hook_input).await?;
        Ok(ControlResponse::Success { result })
    } else {
        Err(ClawError::ControlError(format!("No handler for hook_id: {}", hook_id)))
    }
}
```

## Risks & Mitigations

### 🟢 Low Risk
- **Type definitions** - Straightforward enums/structs from SPEC
  - Mitigation: Follow SPEC.md exactly, comprehensive serde tests

### 🟡 Medium Risk
- **Closure blanket impl** - Complex generic constraints
  - Mitigation: Reference existing async-trait patterns, thorough testing
  - Test with both trait objects and closures

- **Hook matching logic** - Wildcard support complexity
  - Mitigation: Start with exact matching, defer wildcards to future enhancement
  - Document limitation clearly

### 🔴 High Risk
- **Control protocol integration** - Bridge between two handler traits
  - Mitigation: Careful signature design, integration tests
  - Keep HookHandler simple for control protocol
  - Add richer HookCallback in hooks module for SDK users

- **HookInput/HookContext design** - No clear spec
  - Mitigation: Start minimal, extend as needed
  - Make fields optional for forward compatibility

## Success Criteria

✅ **Acceptance Criteria from Task:**

1. **HookEvent enum** - Define events that can trigger hooks
2. **HookMatcher** - Pattern matching for hook triggers
3. **HookCallback trait** with blanket impl for closures
4. **HookResponse** with permission decisions
5. **Hook invocation routing** from control protocol callbacks
6. **Comprehensive tests** (~20 unit tests + integration)
7. **Complete documentation** with examples

✅ **Technical Requirements:**

- All 141 existing tests continue to pass (no regressions)
- 0 clippy warnings in new hook code
- 100% documentation coverage of public API
- Clean compilation under 5 seconds
- Integration with control protocol tested

✅ **Code Quality:**

- Thread-safe where needed (Arc/Mutex patterns)
- Ergonomic API with builder patterns
- Comprehensive error handling
- Production-ready with examples

## Next Steps

1. ✅ Investigation complete
2. Phase 1: Implement core types (HookEvent, HookMatcher, HookResponse)
3. Phase 2: Implement HookCallback trait with blanket impl
4. Phase 3: Create hooks module structure
5. Phase 4: Integrate with control protocol
6. Phase 5: Write comprehensive tests
7. Phase 6: Add documentation and examples
8. Phase 7: Verify all acceptance criteria
9. Phase 8: Run full test suite and clippy
10. Phase 9: Update task status and push

---

**Investigation Status:** ✅ COMPLETE
**Ready to Implement:** YES
**Estimated Duration:** 7.5 hours
**Unblocks:** rusty_claw-s8q (permission management)

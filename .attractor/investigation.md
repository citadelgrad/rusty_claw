# Investigation: rusty_claw-dss - Implement ClaudeAgentOptions builder

**Task ID:** rusty_claw-dss
**Status:** in_progress
**Priority:** P2 (Medium)

## Summary

Implement `ClaudeAgentOptions` struct with builder pattern to provide flexible configuration for Claude agent sessions. This is a foundational configuration type that will enable downstream Control Protocol handler implementation (rusty_claw-91n).

## Current State

### ✅ Completed Dependencies
- **rusty_claw-pwc**: All shared types and message structs are complete
  - `Message`, `SystemMessage`, `AssistantMessage`, `ResultMessage` defined
  - `ContentBlock`, `ToolInfo`, `McpServerInfo`, `UsageInfo` defined
  - Message deserialization working correctly

### 📂 Existing Files
- `crates/rusty_claw/src/lib.rs` - Public API with module structure
- `crates/rusty_claw/src/messages.rs` - Complete message type definitions (627 lines)
- `crates/rusty_claw/src/query.rs` - Uses `Option<()>` as placeholder for options (line 116)
- `crates/rusty_claw/src/transport/mod.rs` - Transport trait definition
- `crates/rusty_claw/src/transport/subprocess.rs` - SubprocessCLITransport implementation
- `crates/rusty_claw/src/error.rs` - ClawError type hierarchy

### ❌ Missing Files
- `crates/rusty_claw/src/options.rs` - **NEW FILE NEEDED**

### 📋 Specification Reference
- **SPEC.md Section 5.1**: ClaudeAgentOptions definition (lines 363-446)
- **SPEC.md Section 2.2**: SubprocessCLITransport CLI arguments (lines 139-151)
- **SPEC.md Section 4.3**: Control Protocol initialization fields (lines 332-338)

## What Needs to Be Done

### 1. Create `crates/rusty_claw/src/options.rs` (NEW FILE, ~400 lines)

Implement the following types:

#### Core Options Struct
```rust
#[derive(Debug, Clone, Default)]
pub struct ClaudeAgentOptions {
    // Prompt & behavior
    pub system_prompt: Option<SystemPrompt>,
    pub append_system_prompt: Option<String>,
    pub max_turns: Option<u32>,
    pub model: Option<String>,

    // Tools & permissions
    pub allowed_tools: Vec<String>,
    pub disallowed_tools: Vec<String>,
    pub permission_mode: Option<PermissionMode>,
    pub permission_prompt_tool_allowlist: Vec<String>,

    // MCP (placeholder for future tasks)
    pub mcp_servers: HashMap<String, McpServerConfig>,
    pub sdk_mcp_servers: Vec<SdkMcpServer>,

    // Hooks (placeholder for future tasks)
    pub hooks: HashMap<HookEvent, Vec<HookMatcher>>,

    // Subagents (placeholder for future tasks)
    pub agents: HashMap<String, AgentDefinition>,

    // Session
    pub resume: Option<String>,
    pub fork_session: bool,
    pub session_name: Option<String>,
    pub enable_file_checkpointing: bool,

    // Environment
    pub cwd: Option<PathBuf>,
    pub cli_path: Option<PathBuf>,
    pub env: HashMap<String, String>,

    // Settings isolation
    pub settings_sources: Option<Vec<String>>,

    // Output
    pub output_format: Option<serde_json::Value>,
    pub include_partial_messages: bool,

    // Advanced
    pub betas: Vec<String>,
    pub sandbox_settings: Option<SandboxSettings>,
}
```

#### Supporting Enums
```rust
pub enum SystemPrompt {
    Custom(String),
    Preset { preset: String },
}

pub enum PermissionMode {
    Default,
    AcceptEdits,
    BypassPermissions,
    Plan,
}
```

#### Placeholder Structs (for future tasks)
```rust
// Will be fully implemented in future MCP tasks
pub struct McpServerConfig {
    // Placeholder - detailed in SPEC.md section 7.1
}

pub struct SdkMcpServer {
    // Placeholder - detailed in SPEC.md section 7.2
}

// Will be fully implemented in future hook tasks
pub struct HookEvent;
pub struct HookMatcher;

// Will be fully implemented in future agent tasks
pub struct AgentDefinition {
    pub description: String,
    pub prompt: String,
    pub tools: Vec<String>,
    pub model: Option<String>,
}

// Will be fully implemented in future sandbox tasks
pub struct SandboxSettings;
```

#### Builder Implementation
```rust
pub struct ClaudeAgentOptionsBuilder {
    inner: ClaudeAgentOptions,
}

impl ClaudeAgentOptions {
    pub fn builder() -> ClaudeAgentOptionsBuilder {
        ClaudeAgentOptionsBuilder::default()
    }
}

impl ClaudeAgentOptionsBuilder {
    // Chainable setters for all fields
    pub fn system_prompt(mut self, prompt: SystemPrompt) -> Self { ... }
    pub fn max_turns(mut self, turns: u32) -> Self { ... }
    pub fn model(mut self, model: impl Into<String>) -> Self { ... }
    pub fn allowed_tools(mut self, tools: Vec<String>) -> Self { ... }
    pub fn permission_mode(mut self, mode: PermissionMode) -> Self { ... }
    pub fn cwd(mut self, path: impl Into<PathBuf>) -> Self { ... }
    // ... (all other fields)

    pub fn build(self) -> ClaudeAgentOptions { ... }
}
```

#### CLI Arguments Conversion
```rust
impl ClaudeAgentOptions {
    /// Convert options to Claude CLI arguments
    pub fn to_cli_args(&self, prompt: &str) -> Vec<String> {
        let mut args = vec![
            "--output-format=stream-json".to_string(),
            "--verbose".to_string(),
        ];

        if let Some(max_turns) = self.max_turns {
            args.push(format!("--max-turns={}", max_turns));
        }

        if let Some(model) = &self.model {
            args.push(format!("--model={}", model));
        }

        if let Some(mode) = &self.permission_mode {
            args.push(format!("--permission-mode={}", mode.to_cli_arg()));
        }

        // Settings isolation for reproducibility
        args.push("--settings-sources=".to_string());

        // Enable control protocol input
        args.push("--input-format=stream-json".to_string());

        // Prompt
        args.push("-p".to_string());
        args.push(prompt.to_string());

        args
    }
}
```

### 2. Modify `crates/rusty_claw/src/lib.rs` (+2 lines)

Add options module:
```rust
/// Configuration options and builder
pub mod options;
```

Update prelude:
```rust
pub use crate::options::{ClaudeAgentOptions, PermissionMode, SystemPrompt};
```

### 3. Modify `crates/rusty_claw/src/query.rs` (~10 lines changed)

Update function signature and implementation:
```rust
// Line 116: Change signature
pub async fn query(
    prompt: impl Into<String>,
    options: Option<ClaudeAgentOptions>,
) -> Result<impl Stream<Item = Result<Message, ClawError>>, ClawError> {
    let prompt = prompt.into();

    // Extract CLI args from options or use defaults
    let args = if let Some(opts) = options {
        opts.to_cli_args(&prompt)
    } else {
        vec![
            "--output-format=stream-json".to_string(),
            "--verbose".to_string(),
            "-p".to_string(),
            prompt,
        ]
    };

    // ... rest of function unchanged
}
```

### 4. Add Tests in `crates/rusty_claw/src/options.rs` (~150 lines)

#### Unit Tests:
- ✅ Builder pattern usage
- ✅ Default values
- ✅ Field setters (chaining)
- ✅ CLI args conversion for all fields
- ✅ PermissionMode enum variants
- ✅ SystemPrompt enum variants
- ✅ Empty options produces minimal CLI args
- ✅ Complex options with all fields set

Example test structure:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_default() {
        let opts = ClaudeAgentOptions::builder().build();
        assert_eq!(opts.max_turns, None);
        assert_eq!(opts.model, None);
        assert!(opts.allowed_tools.is_empty());
    }

    #[test]
    fn test_builder_chaining() {
        let opts = ClaudeAgentOptions::builder()
            .max_turns(5)
            .model("claude-sonnet-4")
            .allowed_tools(vec!["Read".to_string(), "Bash".to_string()])
            .permission_mode(PermissionMode::AcceptEdits)
            .build();

        assert_eq!(opts.max_turns, Some(5));
        assert_eq!(opts.model, Some("claude-sonnet-4".to_string()));
        assert_eq!(opts.allowed_tools.len(), 2);
    }

    #[test]
    fn test_to_cli_args_minimal() {
        let opts = ClaudeAgentOptions::default();
        let args = opts.to_cli_args("test prompt");

        assert!(args.contains(&"--output-format=stream-json".to_string()));
        assert!(args.contains(&"--verbose".to_string()));
        assert!(args.contains(&"--input-format=stream-json".to_string()));
        assert!(args.contains(&"--settings-sources=".to_string()));
        assert!(args.contains(&"-p".to_string()));
        assert!(args.contains(&"test prompt".to_string()));
    }

    #[test]
    fn test_to_cli_args_all_fields() {
        let opts = ClaudeAgentOptions::builder()
            .max_turns(10)
            .model("claude-opus-4")
            .permission_mode(PermissionMode::Plan)
            .build();

        let args = opts.to_cli_args("test");

        assert!(args.contains(&"--max-turns=10".to_string()));
        assert!(args.contains(&"--model=claude-opus-4".to_string()));
        assert!(args.contains(&"--permission-mode=plan".to_string()));
    }

    #[test]
    fn test_permission_mode_to_cli_arg() {
        assert_eq!(PermissionMode::Default.to_cli_arg(), "default");
        assert_eq!(PermissionMode::AcceptEdits.to_cli_arg(), "accept-edits");
        assert_eq!(PermissionMode::BypassPermissions.to_cli_arg(), "bypass-permissions");
        assert_eq!(PermissionMode::Plan.to_cli_arg(), "plan");
    }
}
```

### 5. Add Documentation (~50 lines)

Add module-level documentation with examples:
```rust
//! Configuration options and builder pattern for Claude agent sessions
//!
//! This module provides [`ClaudeAgentOptions`] for configuring Claude agent behavior,
//! including prompt settings, tools, permissions, session management, and environment.
//!
//! # Example
//!
//! ```
//! use rusty_claw::options::{ClaudeAgentOptions, PermissionMode, SystemPrompt};
//!
//! let options = ClaudeAgentOptions::builder()
//!     .allowed_tools(vec!["Read".to_string(), "Bash".to_string()])
//!     .permission_mode(PermissionMode::AcceptEdits)
//!     .max_turns(5)
//!     .model("claude-sonnet-4")
//!     .build();
//! ```
//!
//! # Builder Pattern
//!
//! All fields have sensible defaults. Use the builder pattern for convenient configuration:
//!
//! ```
//! let options = ClaudeAgentOptions::builder()
//!     .system_prompt(SystemPrompt::Custom("You are a helpful assistant".to_string()))
//!     .max_turns(10)
//!     .build();
//! ```
```

## Design Decisions

### 1. **Builder Pattern**
- **Choice:** Hand-rolled builder instead of `derive_builder` crate
- **Rationale:**
  - Zero additional dependencies (aligns with SPEC.md section 11.1 "Minimal surface")
  - Simple to implement (~100 lines)
  - Full control over builder API
  - No proc macro compile time cost

### 2. **Placeholder Types**
- **Choice:** Create minimal placeholder structs for MCP, hooks, agents
- **Rationale:**
  - Enables complete ClaudeAgentOptions API surface now
  - Downstream tasks will flesh out these types
  - Prevents breaking API changes later
  - Follows SPEC.md structure

### 3. **CLI Arguments Conversion**
- **Choice:** Implement `to_cli_args()` method on ClaudeAgentOptions
- **Rationale:**
  - Central place for CLI argument logic
  - Used by `query()` function now
  - Will be used by `ClaudeClient` in future tasks
  - Follows SPEC.md section 2.2 CLI argument spec

### 4. **Default Values**
- **Choice:** Use `Default` trait with `None` for optional fields, empty collections
- **Rationale:**
  - Sensible defaults for all fields
  - Follows Rust conventions
  - Enables partial configuration
  - No required fields (except prompt in query())

### 5. **Permission Mode Naming**
- **Choice:** Use Rust enum names: `Default`, `AcceptEdits`, `BypassPermissions`, `Plan`
- **Rationale:**
  - Follows Rust naming conventions (PascalCase)
  - Implements `to_cli_arg()` for snake-case conversion
  - Matches SPEC.md section 5.1 definition

## File Changes Summary

### New Files (1):
1. `crates/rusty_claw/src/options.rs` (~400 lines)
   - ClaudeAgentOptions struct
   - ClaudeAgentOptionsBuilder
   - Supporting enums and placeholder types
   - CLI args conversion
   - Comprehensive tests

### Modified Files (2):
1. `crates/rusty_claw/src/lib.rs` (+4 lines)
   - Add `pub mod options;`
   - Update prelude exports

2. `crates/rusty_claw/src/query.rs` (~10 lines changed)
   - Change `options` parameter from `Option<()>` to `Option<ClaudeAgentOptions>`
   - Use `options.to_cli_args()` instead of hardcoded args

## Risk Analysis

### 🟢 Low Risk Areas
1. **Pure Additive Changes**: New module, no modifications to existing types
2. **Default Trait**: All optional fields, backward compatible
3. **Builder Pattern**: Standard Rust idiom, well-tested pattern
4. **Existing Tests**: No impact on existing 29 passing tests

### 🟡 Medium Risk Areas
1. **Placeholder Types**: Future tasks must implement full types
   - **Mitigation**: Document clearly which fields are placeholders
   - **Mitigation**: Use Rust visibility (`pub(crate)` initially) for unstable types

2. **CLI Args Conversion**: Must match Claude CLI exactly
   - **Mitigation**: Follow SPEC.md section 2.2 precisely
   - **Mitigation**: Add comprehensive tests for all CLI args
   - **Mitigation**: Test with real Claude CLI in integration tests

### 🔴 High Risk Areas
None identified. This is a foundational configuration type with no side effects.

## Dependencies & Blockers

### ✅ Satisfied Dependencies
- **rusty_claw-pwc**: Complete (all message types defined)
- **Cargo dependencies**: All required crates available (serde, serde_json, std collections)

### ⚠️ Blocks Downstream Tasks
- **rusty_claw-91n** [P1]: Implement Control Protocol handler
  - Requires ClaudeAgentOptions for initialization
  - Uses `hooks`, `agents`, `sdk_mcp_servers` fields

## Testing Strategy

### Unit Tests (in options.rs)
1. ✅ Builder default values
2. ✅ Builder chaining
3. ✅ All field setters
4. ✅ CLI args conversion (minimal options)
5. ✅ CLI args conversion (all fields)
6. ✅ PermissionMode enum to CLI arg conversion
7. ✅ SystemPrompt enum variants
8. ✅ Collections (Vec, HashMap) handling
9. ✅ PathBuf conversion
10. ✅ Optional fields handling

### Integration Tests (future)
- Test with real Claude CLI (verify CLI args work correctly)
- Test with mock CLI (verify options are passed through transport)

### Documentation Tests
- Ensure all examples in module docs compile and run

## Success Criteria

1. ✅ **ClaudeAgentOptions struct created** with all fields from SPEC.md section 5.1
2. ✅ **Builder pattern implemented** with chainable setters
3. ✅ **CLI args conversion** working for all fields
4. ✅ **Supporting enums** (SystemPrompt, PermissionMode) implemented
5. ✅ **Placeholder types** created for future tasks (MCP, hooks, agents)
6. ✅ **query() function updated** to use ClaudeAgentOptions
7. ✅ **Comprehensive tests** (10+ unit tests)
8. ✅ **Zero clippy warnings** in options.rs
9. ✅ **All existing tests pass** (no regressions)
10. ✅ **Complete documentation** with examples

## Implementation Phases

### Phase 1: Core Options Struct (~90 minutes)
1. Create `options.rs` file
2. Define `ClaudeAgentOptions` struct with all fields
3. Implement `Default` trait
4. Add supporting enums (SystemPrompt, PermissionMode)
5. Create placeholder types (McpServerConfig, etc.)
6. Write module-level documentation

### Phase 2: Builder Pattern (~60 minutes)
1. Define `ClaudeAgentOptionsBuilder` struct
2. Implement all setter methods (chainable)
3. Implement `build()` method
4. Add builder documentation and examples

### Phase 3: CLI Args Conversion (~45 minutes)
1. Implement `to_cli_args()` method
2. Handle all option fields correctly
3. Follow SPEC.md section 2.2 CLI arguments
4. Add helper methods (e.g., `PermissionMode::to_cli_arg()`)

### Phase 4: Integration (~30 minutes)
1. Add `pub mod options;` to lib.rs
2. Update prelude exports
3. Modify `query()` function in query.rs
4. Update query() function signature and docs

### Phase 5: Testing (~90 minutes)
1. Write builder tests
2. Write CLI args tests
3. Write enum conversion tests
4. Write edge case tests
5. Run full test suite
6. Fix any clippy warnings

### Phase 6: Documentation & Verification (~30 minutes)
1. Write usage examples
2. Add inline documentation
3. Verify all doc tests compile
4. Run final test suite
5. Verify against acceptance criteria

**Total Estimated Effort:** ~5.5 hours

## Unblocks

Completing this task will unblock:
- **rusty_claw-91n** [P1]: Implement Control Protocol handler
  - Needs ClaudeAgentOptions for control protocol initialization
  - Uses hooks, agents, and sdk_mcp_servers configuration

## Next Steps

1. ✅ Create `crates/rusty_claw/src/options.rs` (Phase 1)
2. ✅ Implement builder pattern (Phase 2)
3. ✅ Implement CLI args conversion (Phase 3)
4. ✅ Integrate with lib.rs and query.rs (Phase 4)
5. ✅ Add comprehensive tests (Phase 5)
6. ✅ Document and verify (Phase 6)

## References

- **SPEC.md Section 5.1**: ClaudeAgentOptions definition (lines 363-446)
- **SPEC.md Section 2.2**: SubprocessCLITransport CLI arguments (lines 139-151)
- **SPEC.md Section 4.3**: Control Protocol initialization (lines 332-338)
- **SPEC.md Section 11.1**: Dependency Philosophy - "Minimal surface"
- **Python SDK Reference**: `claude-agent-sdk-python` (MIT License)

---

**Investigation Complete** ✅
Ready to proceed with implementation!

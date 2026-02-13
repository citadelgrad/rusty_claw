# Investigation: Define error hierarchy (rusty_claw-9pf)

**Date:** 2026-02-13
**Task ID:** rusty_claw-9pf
**Status:** IN_PROGRESS
**Priority:** P1 (Critical path)

---

## Task Overview

Implement the complete error hierarchy for the rusty_claw SDK using `thiserror`. This is a critical foundational task that blocks three major implementation tasks:
- rusty_claw-6cn: Transport trait needs `ClawError` for all method signatures
- rusty_claw-pwc: Message parsing needs `MessageParse` variant
- rusty_claw-k71: CLI discovery needs `CliNotFound` and `Process` variants

## Current State

### Existing Code

**File:** `crates/rusty_claw/src/lib.rs:64-66`
```rust
/// Error types and utilities
pub mod error {
    //! Error hierarchy will be added in future tasks
}
```

The error module exists but is currently empty. The module is properly declared and documented in the public API.

**Dependencies:** `thiserror` is already added to `Cargo.toml` (line 19) via workspace inheritance.

### Specification Reference

From `docs/SPEC.md:664-703`, the complete error enum is defined with 9 variants:

```rust
#[derive(Error, Debug)]
pub enum ClawError {
    #[error("Claude Code CLI not found. Install it or set cli_path.")]
    CliNotFound,

    #[error("Failed to connect to Claude Code CLI: {0}")]
    Connection(String),

    #[error("CLI process exited with code {code}: {stderr}")]
    Process {
        code: i32,
        stderr: String,
    },

    #[error("Failed to parse JSON from CLI: {0}")]
    JsonDecode(#[from] serde_json::Error),

    #[error("Failed to parse message: {reason}")]
    MessageParse {
        reason: String,
        raw: String,
    },

    #[error("Control protocol timeout waiting for {subtype}")]
    ControlTimeout {
        subtype: String,
    },

    #[error("Control protocol error: {0}")]
    ControlError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Tool execution failed: {0}")]
    ToolExecution(String),
}
```

## Required Changes

### File to Modify

**Path:** `crates/rusty_claw/src/error.rs` (new file to create)

**Reason:** Rust convention is to implement module contents in separate files when they contain substantial code. The empty `pub mod error { }` in `lib.rs` will automatically look for `error.rs` in the `src/` directory.

### Implementation Requirements

1. **Create `error.rs` file** with:
   - Import statement: `use thiserror::Error;`
   - Complete `ClawError` enum with all 9 variants
   - Proper `thiserror` attributes for each variant
   - Comprehensive documentation

2. **Update `lib.rs`**:
   - Remove the empty inline module at line 64-66
   - Replace with: `pub mod error;` (which will import from `error.rs`)
   - Add re-export in `prelude` module for convenience

3. **Error Variant Analysis**:

   | Variant | Type | Purpose | Auto-conversion |
   |---------|------|---------|-----------------|
   | `CliNotFound` | Unit | CLI binary not found during discovery | No |
   | `Connection(String)` | Tuple | Transport connection failures | No |
   | `Process { code, stderr }` | Struct | CLI process crashes or non-zero exits | No |
   | `JsonDecode` | From | JSONL parsing errors | Yes (from serde_json::Error) |
   | `MessageParse { reason, raw }` | Struct | Malformed control protocol messages | No |
   | `ControlTimeout { subtype }` | Struct | Control protocol request timeouts | No |
   | `ControlError(String)` | Tuple | Control protocol semantic errors | No |
   | `Io` | From | Filesystem and I/O operations | Yes (from std::io::Error) |
   | `ToolExecution(String)` | Tuple | MCP tool handler failures | No |

4. **Special Considerations**:
   - Two variants use `#[from]` attribute for automatic conversion:
     - `JsonDecode` from `serde_json::Error`
     - `Io` from `std::io::Error`
   - This enables `?` operator to automatically convert these errors
   - All error messages follow clear, actionable format

## Dependencies

### Satisfied
- ✅ `thiserror` crate is available (workspace dependency)
- ✅ Workspace structure is set up (rusty_claw-eia completed)
- ✅ Module structure defined in `lib.rs`

### None Required
This task is purely additive and has no external blockers.

## Risks

### Low Risk Factors

1. **API Stability**: Error enum is well-specified in SPEC.md and follows standard Rust patterns
2. **Backwards Compatibility**: This is the initial implementation, no existing API to maintain
3. **Testing**: Error types can be unit tested in isolation without complex setup

### Potential Issues

1. **Error Message Quality**: Messages should be:
   - Clear for users
   - Actionable (suggest remediation)
   - Consistent in tone
   - Free of implementation details

   **Mitigation**: Follow the exact messages from SPEC.md which have been designed for clarity

2. **Future Error Variants**: May need to add more variants as features are implemented

   **Mitigation**: Rust's exhaustive pattern matching will catch any missing cases during implementation of dependent modules

## Implementation Strategy

### Step 1: Create error.rs
- Copy the complete error enum from SPEC.md
- Add module-level documentation
- Include usage examples in doc comments

### Step 2: Update lib.rs
- Replace inline `error` module with file-based module
- Verify module is public and properly exported

### Step 3: Update prelude
- Add `pub use crate::error::ClawError;` to prelude module
- This enables `use rusty_claw::prelude::*;` to include error types

### Step 4: Verification
- Run `cargo check` to verify compilation
- Run `cargo clippy` to ensure code quality
- Run `cargo doc` to verify documentation renders correctly
- Verify the error can be constructed and displayed:
  ```rust
  let err = ClawError::CliNotFound;
  assert_eq!(err.to_string(), "Claude Code CLI not found. Install it or set cli_path.");
  ```

## Blocks Downstream Tasks

This task unblocks three critical P1/P2 tasks:

1. **rusty_claw-6cn** [P1]: Transport trait needs `ClawError` for all method signatures
2. **rusty_claw-pwc** [P1]: Message parsing needs `MessageParse` variant
3. **rusty_claw-k71** [P2]: CLI discovery needs `CliNotFound` and `Process` variants

None of these tasks can proceed without the complete error hierarchy in place.

## Testing Strategy

### Unit Tests to Add

Create tests in `crates/rusty_claw/src/error.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_not_found_message() {
        let err = ClawError::CliNotFound;
        assert_eq!(
            err.to_string(),
            "Claude Code CLI not found. Install it or set cli_path."
        );
    }

    #[test]
    fn test_connection_error_message() {
        let err = ClawError::Connection("timeout".to_string());
        assert_eq!(err.to_string(), "Failed to connect to Claude Code CLI: timeout");
    }

    #[test]
    fn test_process_error_message() {
        let err = ClawError::Process {
            code: 1,
            stderr: "permission denied".to_string(),
        };
        assert!(err.to_string().contains("code 1"));
        assert!(err.to_string().contains("permission denied"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let claw_err: ClawError = io_err.into();
        assert!(claw_err.to_string().contains("file not found"));
    }

    #[test]
    fn test_json_error_conversion() {
        let json_str = "{ invalid json }";
        let json_err = serde_json::from_str::<serde_json::Value>(json_str).unwrap_err();
        let claw_err: ClawError = json_err.into();
        assert!(claw_err.to_string().contains("parse"));
    }
}
```

### Verification Checklist

- [ ] `cargo check` passes
- [ ] `cargo clippy` has no warnings
- [ ] `cargo test` passes (all unit tests)
- [ ] `cargo doc` generates clean documentation
- [ ] Error messages match SPEC.md exactly
- [ ] All 9 variants are implemented
- [ ] Both `#[from]` conversions work correctly
- [ ] Module is properly exported in lib.rs
- [ ] ClawError is available in prelude

## Success Criteria

1. ✅ All 9 error variants implemented exactly as specified
2. ✅ Unit tests verify error message formatting
3. ✅ Automatic conversion from `std::io::Error` works
4. ✅ Automatic conversion from `serde_json::Error` works
5. ✅ Module is properly exported and documented
6. ✅ No compiler warnings or clippy issues
7. ✅ Documentation renders correctly in `cargo doc`
8. ✅ Error type is available via prelude import

---

## Next Steps After Completion

1. Mark rusty_claw-9pf as closed
2. Unblocks rusty_claw-6cn (Transport trait implementation)
3. Unblocks rusty_claw-pwc (Shared types and message structs)
4. Unblocks rusty_claw-k71 (CLI discovery)
5. Update `.attractor/current_task.md` with next task from pipeline

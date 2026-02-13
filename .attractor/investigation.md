# Investigation: Set up workspace and crate structure

**Task ID:** rusty_claw-eia
**Status:** IN_PROGRESS
**Date:** 2026-02-12

## Summary

Need to create a Cargo workspace with two crates:
1. `rusty_claw` - main library crate
2. `rusty_claw_macros` - proc-macro crate for `#[claw_tool]` attribute

## Current State

The project directory exists at `/Volumes/qwiizlab/projects/rusty_claw` with:
- Documentation: `docs/PRD.md` and `docs/SPEC.md`
- Agent instructions: `AGENTS.md`
- Beads issue tracking: `.beads/`
- Git repository initialized
- **No Rust code or Cargo files yet** - this is a greenfield setup

## Files to Create

### 1. Workspace Root
- **`/Volumes/qwiizlab/projects/rusty_claw/Cargo.toml`** - Workspace definition

### 2. Main Library Crate
- **`/Volumes/qwiizlab/projects/rusty_claw/crates/rusty_claw/Cargo.toml`** - Library crate manifest
- **`/Volumes/qwiizlab/projects/rusty_claw/crates/rusty_claw/src/lib.rs`** - Initial library entry point

### 3. Proc Macro Crate
- **`/Volumes/qwiizlab/projects/rusty_claw/crates/rusty_claw_macros/Cargo.toml`** - Proc macro crate manifest
- **`/Volumes/qwiizlab/projects/rusty_claw/crates/rusty_claw_macros/src/lib.rs`** - Proc macro entry point

### 4. Additional Files
- **`.gitignore`** - Already exists but may need Rust-specific entries added
- **`README.md`** - Project overview (not strictly required for this task)

## Required Changes

### 1. Workspace Cargo.toml

Create a workspace manifest that:
- Declares members: `crates/rusty_claw` and `crates/rusty_claw_macros`
- Sets workspace-level metadata (edition, license, authors)
- Optionally defines shared dependencies via `[workspace.dependencies]`

### 2. rusty_claw Library Crate

According to SPEC.md section 11, need these core dependencies:
- `tokio` (^1.35, features: `full`) - async runtime
- `serde` (^1, features: `derive`) - serialization
- `serde_json` (^1) - JSON parsing
- `thiserror` (^2) - error macros
- `uuid` (^1, features: `v4`) - request IDs
- `tokio-stream` (^0.1) - stream utilities
- `tracing` (^0.1) - logging
- `async-trait` (^0.1) - async traits

Crate configuration:
- `edition = "2021"` (Rust 2021 edition)
- `name = "rusty_claw"`
- `version = "0.1.0"` (per PRD section 12)
- `license = "MIT"`

### 3. rusty_claw_macros Proc Macro Crate

Proc macro dependencies (SPEC section 11.1):
- `syn` (^2, features: `full`) - Rust syntax parsing
- `quote` (^1) - code generation
- `proc-macro2` (^1) - proc macro utilities

Crate configuration:
- `edition = "2021"`
- `name = "rusty_claw_macros"`
- `version = "0.1.0"`
- `license = "MIT"`
- **CRITICAL:** `proc-macro = true` in `[lib]` section

### 4. Initial Source Files

**rusty_claw/src/lib.rs:**
- Basic module structure with placeholder comments
- Re-export key types (to be implemented later)
- Documentation comments referencing the MIT-licensed Python SDK

**rusty_claw_macros/src/lib.rs:**
- Placeholder proc macro stub
- Will be implemented in future tasks

### 5. .gitignore Updates

Add Rust-specific entries:
```
/target/
**/*.rs.bk
*.pdb
Cargo.lock  # For library crates (keep for binary crates)
```

**Note:** `.gitignore` already exists with basic entries - need to verify it includes Rust patterns.

## Risks & Dependencies

### Risks
1. **None significant** - this is a straightforward workspace setup
2. Cargo version compatibility - using modern Cargo features (workspace inheritance) requires Cargo 1.64+
3. Proc macro crate must have `proc-macro = true` or it won't work

### Dependencies
- **Blocks:** rusty_claw-9pf (Define error hierarchy) - error types will go in this workspace
- **Requires:** Cargo installed locally (assumed to be present)
- **No code dependencies yet** - this is the foundation task

## Implementation Strategy

1. Create directory structure: `crates/rusty_claw/src/` and `crates/rusty_claw_macros/src/`
2. Write workspace `Cargo.toml`
3. Write `rusty_claw/Cargo.toml` with all dependencies from SPEC
4. Write `rusty_claw_macros/Cargo.toml` with proc macro deps
5. Write minimal `lib.rs` files for both crates
6. Update `.gitignore` if needed
7. Run `cargo check` to verify workspace builds
8. Run `cargo tree` to verify dependency resolution
9. Commit changes

## Verification Steps

After implementation:
- [ ] `cargo check` succeeds
- [ ] `cargo build` succeeds
- [ ] `cargo tree` shows correct dependency graph
- [ ] Both crates appear in `cargo metadata` output
- [ ] Workspace structure matches SPEC.md section 1.2

## Notes

- Using Rust 2021 edition (requires rustc >= 1.56.0, SPEC requires 1.75+ for async traits)
- Proc macro crate follows standard naming convention (`<crate>_macros`)
- License is MIT, crediting Anthropic's Python SDK as architectural reference (per PRD 2.2)
- Version 0.1.0 aligns with PRD "Foundation" phase (section 12)

## References

- **PRD.md:** Section 2 (Licensing), Section 3.1 (P0 Requirements), Section 12 (Release Plan)
- **SPEC.md:** Section 1.2 (Crate Structure), Section 11 (Dependencies), Section 13.1 (Rust Considerations)
- **Reference Implementation:** [claude-agent-sdk-python](https://github.com/anthropics/claude-agent-sdk-python) (MIT License)

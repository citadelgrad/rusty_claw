# Investigation: rusty_claw-k71 - Implement CLI discovery and version check

**Task ID:** rusty_claw-k71
**Priority:** P2
**Status:** IN_PROGRESS
**Date:** 2026-02-13

## Overview

Implement CLI discovery logic to find the `claude` binary in multiple locations and validate that its version is >= 2.0.0. This will integrate with the existing `SubprocessCLITransport` we completed in rusty_claw-6cn.

## Current State

### ✅ What Exists
1. **Transport Implementation** (rusty_claw-6cn completed)
   - `SubprocessCLITransport` in `crates/rusty_claw/src/transport/subprocess.rs`
   - Currently takes `cli_path: PathBuf` in constructor (line 100)
   - No CLI discovery - relies on caller providing valid path
   - No version validation

2. **Error Hierarchy** (rusty_claw-9pf completed)
   - `ClawError::CliNotFound` exists (line 52 in error.rs)
   - ❌ Missing: `ClawError::InvalidCliVersion` variant

3. **Dependencies Available**
   - tokio (async runtime)
   - std::process::Command (for running `claude --version`)
   - std::path::PathBuf
   - std::env (for environment variables)

4. **CLI Verification**
   - Confirmed `claude` is in PATH: `/opt/homebrew/bin/claude`, `/Users/scott/.local/bin/claude`
   - Confirmed `claude --version` works: outputs `"2.1.39 (Claude Code)"`
   - Version format: `<semver> (Claude Code)`

### ❌ What's Missing
1. **CliDiscovery struct** - Not implemented
2. **InvalidCliVersion error** - Not in ClawError enum
3. **semver dependency** - Not in Cargo.toml (need for version parsing)
4. **Integration with SubprocessCLITransport** - Constructor doesn't call CliDiscovery

## Implementation Plan

### Phase 1: Add Error Variant (5 min)
**File:** `crates/rusty_claw/src/error.rs`

Add new error variant:
```rust
#[error("Invalid Claude CLI version: expected >= 2.0.0, found {version}")]
InvalidCliVersion { version: String },
```

**Location:** After `CliNotFound` (around line 53)

**Tests to add:**
- `test_invalid_cli_version_message()` - Verify error message formatting

### Phase 2: Add semver Dependency (2 min)
**File:** `Cargo.toml` (workspace root)

Add to `[workspace.dependencies]`:
```toml
semver = "1.0"
```

**File:** `crates/rusty_claw/Cargo.toml`

Add to `[dependencies]`:
```toml
semver = { workspace = true }
```

### Phase 3: Create CliDiscovery Module (30 min)
**File:** `crates/rusty_claw/src/transport/discovery.rs` (NEW)

#### 3.1 Struct Definition
```rust
pub struct CliDiscovery;

impl CliDiscovery {
    pub async fn find(cli_path: Option<&Path>) -> Result<PathBuf, ClawError>
    pub async fn validate_version(cli: &Path) -> Result<String, ClawError>
}
```

#### 3.2 Search Strategy (find method)
Search in order (stop at first success):
1. **Explicit cli_path argument** - If provided, check if exists
2. **CLAUDE_CLI_PATH env var** - Check `std::env::var("CLAUDE_CLI_PATH")`
3. **PATH search** - Use `which::which("claude")` (need `which` crate)
4. **Common locations** - Hardcoded paths:
   - `/opt/homebrew/bin/claude` (macOS Homebrew)
   - `/usr/local/bin/claude` (Manual install)
   - `~/.local/bin/claude` (User install)
   - `~/.npm/bin/claude` (npm global install)
   - `~/.claude/local/claude` (Claude Code self-install)

**Error:** Return `ClawError::CliNotFound` if all searches fail

#### 3.3 Version Validation (validate_version method)
1. Run: `tokio::process::Command::new(cli).arg("--version").output().await`
2. Parse stdout: Extract version string from `"X.Y.Z (Claude Code)"`
3. Use `semver::Version::parse()` to parse version
4. Compare: `version >= Version::new(2, 0, 0)`
5. Return `Ok(version_string)` or `Err(ClawError::InvalidCliVersion)`

**Errors:**
- I/O error → `ClawError::Io`
- Parse failure → `ClawError::InvalidCliVersion`
- Version < 2.0.0 → `ClawError::InvalidCliVersion`

#### 3.4 Tests to Add
- `test_find_with_explicit_path()` - Explicit cli_path takes precedence
- `test_find_with_env_var()` - CLAUDE_CLI_PATH env var works
- `test_find_in_path()` - Searches PATH correctly
- `test_find_not_found()` - Returns CliNotFound when missing
- `test_validate_version_valid()` - Version >= 2.0.0 passes
- `test_validate_version_invalid()` - Version < 2.0.0 fails
- `test_validate_version_parse_error()` - Malformed output fails

### Phase 4: Integrate with SubprocessCLITransport (15 min)
**File:** `crates/rusty_claw/src/transport/subprocess.rs`

#### 4.1 Update Constructor
**Current (line 100-110):**
```rust
pub fn new(cli_path: PathBuf, args: Vec<String>) -> Self
```

**New signature:**
```rust
pub fn new(cli_path: Option<PathBuf>, args: Vec<String>) -> Self
```

**Store both:**
```rust
struct SubprocessCLITransport {
    /// Optional CLI path (resolved on connect if None)
    cli_path_arg: Option<PathBuf>,
    /// Resolved CLI path (set during connect)
    cli_path: Arc<Mutex<Option<PathBuf>>>,
    // ... rest of fields
}
```

#### 4.2 Update connect() Method
**Current (line 278-336):** Uses `self.cli_path` directly

**New behavior (in connect):**
1. Resolve CLI path if not set:
   ```rust
   let cli_path = if let Some(mut guard) = self.cli_path.lock().await {
       if guard.is_none() {
           let discovered = CliDiscovery::find(self.cli_path_arg.as_deref()).await?;
           *guard = Some(discovered.clone());
           discovered
       } else {
           guard.as_ref().unwrap().clone()
       }
   } else { ... };
   ```
2. Call `CliDiscovery::validate_version(&cli_path).await?`
3. Then spawn subprocess with validated path

**Tests to update:**
- Update all tests to pass `None` for cli_path (use discovery)
- Add `test_connect_validates_version()` - Ensures version check happens
- Update `test_connect_with_invalid_cli()` to use explicit bad path

### Phase 5: Export and Document (5 min)
**File:** `crates/rusty_claw/src/transport/mod.rs`

Add module:
```rust
mod discovery;
pub use discovery::CliDiscovery;
```

Update module docs to mention CLI discovery

## Dependencies

### ✅ Satisfied
- rusty_claw-9pf (error hierarchy) - COMPLETED

### ⚠️ New Dependencies Required
- `semver = "1.0"` - For semantic version parsing
- `which = "6.0"` (OPTIONAL) - For PATH search (can use manual implementation if desired)

## Risks & Considerations

### 🟡 Medium Risk: PATH Search Implementation
**Issue:** Need to search PATH environment variable for `claude` binary

**Options:**
1. Use `which` crate (simplest, adds dependency)
2. Manual implementation:
   ```rust
   std::env::var("PATH")
       .split(':')
       .map(|dir| Path::new(dir).join("claude"))
       .find(|path| path.exists())
   ```

**Recommendation:** Use `which` crate - it's battle-tested and handles platform differences

### 🟢 Low Risk: Home Directory Expansion
**Issue:** Common locations like `~/.local/bin/claude` need tilde expansion

**Solution:**
```rust
use std::env;
let home = env::var("HOME").unwrap_or_default();
let path = PathBuf::from(home).join(".local/bin/claude");
```

**Alternative:** Use `dirs` crate for cross-platform home dir

### 🟢 Low Risk: Version String Parsing
**Issue:** Version format is `"X.Y.Z (Claude Code)"`, need to extract X.Y.Z

**Solution:**
```rust
let output = String::from_utf8_lossy(&output.stdout);
let version_str = output
    .split_whitespace()
    .next()
    .ok_or(ClawError::InvalidCliVersion { version: output.to_string() })?;
semver::Version::parse(version_str)?
```

### 🟢 Low Risk: Backwards Compatibility
**Issue:** Changing `SubprocessCLITransport::new()` signature from required to optional PathBuf

**Impact:**
- Breaking change for existing code
- Tests need updates

**Mitigation:**
- We're at 0.1.0 (pre-release), breaking changes are acceptable
- Update all tests in same commit
- Clear migration notes in CHANGELOG

## Success Criteria

### Code Changes
- ✅ Add `ClawError::InvalidCliVersion` variant with test
- ✅ Add `semver` dependency to Cargo.toml
- ✅ Create `crates/rusty_claw/src/transport/discovery.rs` with CliDiscovery
- ✅ Export CliDiscovery from `transport/mod.rs`
- ✅ Update SubprocessCLITransport constructor to take `Option<PathBuf>`
- ✅ Update SubprocessCLITransport::connect() to call CliDiscovery

### Test Coverage
- ✅ Error variant test in error.rs
- ✅ 7+ tests in discovery.rs covering all search paths
- ✅ Update existing transport tests for new signature
- ✅ Add version validation test in subprocess.rs

### Quality Gates
- ✅ All tests pass (including existing 37 tests)
- ✅ Zero clippy warnings
- ✅ Documentation complete (discovery module and methods)
- ✅ SPEC.md compliance (lines 712-729)

### Integration
- ✅ SubprocessCLITransport automatically discovers CLI on connect
- ✅ Validates version >= 2.0.0 before spawning
- ✅ Returns clear errors for missing CLI or invalid version

## Next Steps (After Completion)

This task **unblocks**:
- **rusty_claw-sna** [P1]: Implement query() function
  - query() will use SubprocessCLITransport
  - Can now rely on automatic CLI discovery

## Files to Create
1. `crates/rusty_claw/src/transport/discovery.rs` - New module

## Files to Modify
1. `crates/rusty_claw/src/error.rs` - Add InvalidCliVersion variant
2. `Cargo.toml` - Add semver (and optionally which) dependency
3. `crates/rusty_claw/Cargo.toml` - Add semver dependency
4. `crates/rusty_claw/src/transport/mod.rs` - Export CliDiscovery
5. `crates/rusty_claw/src/transport/subprocess.rs` - Integrate CliDiscovery

## Estimated Effort
- **Total:** ~60 minutes
- **Breakdown:**
  - Error variant: 5 min
  - Dependencies: 2 min
  - CliDiscovery module: 30 min
  - SubprocessCLITransport integration: 15 min
  - Documentation and exports: 5 min
  - Testing and verification: 10 min (covered by test runs)

## Notes

- The `claude --version` output format is stable: `"2.1.39 (Claude Code)"`
- We can assume this format won't change in 2.x versions
- If format changes, version parsing will fail safely with InvalidCliVersion
- The search order prioritizes explicit configuration over automatic discovery
- This follows Unix conventions (explicit > env var > PATH > common locations)

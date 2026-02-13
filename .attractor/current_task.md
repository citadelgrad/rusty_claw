# Current Task: rusty_claw-k71

**Task ID:** rusty_claw-k71
**Status:** IN_PROGRESS
**Priority:** P2
**Type:** Task
**Owner:** Scott Nixon

## Title
Implement CLI discovery and version check

## Description
Implement CliDiscovery with find() searching cli_path, CLAUDE_CLI_PATH env, PATH, and common install locations. Add validate_version() to ensure CLI >= 2.0.0 via semver parsing.

## Dependencies
- ✅ Depends on: rusty_claw-9pf (Define error hierarchy) - COMPLETED

## Blocks
- ○ rusty_claw-sna (Implement query() function) - P1

## Key Requirements

1. **CliDiscovery struct with find() method:**
   - Takes optional cli_path argument
   - Searches in order: cli_path → CLAUDE_CLI_PATH env → PATH → common locations
   - Returns Result<PathBuf, ClawError::CliNotFound>

2. **validate_version() function:**
   - Executes CLI with --version flag
   - Parses semantic version string
   - Ensures version >= 2.0.0
   - Returns Result<(), ClawError::InvalidCliVersion>

3. **Common locations to search:**
   - /opt/homebrew/bin/claude (macOS)
   - /usr/local/bin/claude
   - ~/.local/bin/claude
   - /usr/bin/claude

4. **Integration with SubprocessCLITransport:**
   - Use CliDiscovery::find() in transport constructor
   - Call validate_version() on connect

## Files to Create/Modify
- Create/Modify: `crates/rusty_claw/src/transport/discovery.rs` - CliDiscovery implementation
- Modify: `crates/rusty_claw/src/transport/mod.rs` - Export discovery module
- Modify: `crates/rusty_claw/src/transport/subprocess.rs` - Integrate CliDiscovery

## Reference
- SPEC.md: CLI discovery specification
- Error hierarchy: `crates/rusty_claw/src/error.rs`
- Existing transport implementation

## Test Coverage Needed
- CLI discovery in PATH
- CLI discovery with CLAUDE_CLI_PATH env var
- CLI discovery with explicit cli_path argument
- Missing CLI returns CliNotFound error
- Invalid version returns InvalidCliVersion error
- Valid version >= 2.0.0 passes validation

## Success Criteria
- ✅ CliDiscovery struct with find() method
- ✅ validate_version() function with semver parsing
- ✅ All search locations checked in correct order
- ✅ Integration with SubprocessCLITransport
- ✅ All unit tests passing
- ✅ Zero clippy warnings
- ✅ Documentation complete

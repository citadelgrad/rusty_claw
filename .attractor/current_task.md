# Current Task: rusty_claw-zyo

**Task ID:** rusty_claw-zyo
**Title:** Implement #[claw_tool] proc macro
**Priority:** P2 (High)
**Type:** Task
**Status:** IN_PROGRESS
**Owner:** Scott Nixon

## Description

In rusty_claw_macros crate, implement the #[claw_tool] attribute macro that generates SdkMcpTool definitions with auto-derived input_schema from function parameters and a ToolHandler impl wrapping the function body.

## Dependencies

**Depends on:**
- ✅ **rusty_claw-tlh** - Implement SDK MCP Server bridge [P2] - COMPLETE

**Blocks:**
- ○ **rusty_claw-5uw** - Documentation and crates.io prep [P3] - OPEN

## Acceptance Criteria

The task should deliver:
1. Functional #[claw_tool] macro in rusty_claw_macros crate
2. Auto-derive input_schema from function parameters
3. Generate SdkMcpTool struct definitions
4. Generate ToolHandler impl wrapping function body
5. Validate JSON-serializable parameters
6. Handle error cases gracefully
7. Integration tests with SDK
8. Zero clippy warnings
9. Comprehensive documentation/examples

## What This Task Unblocks

- ○ **rusty_claw-5uw** - Documentation and crates.io prep

## Epic Context

Part of epic **rusty_claw-sz6**: Rust implementation of the Claude Agent SDK, providing Transport, Control Protocol, MCP integration, hooks, and proc macros for building Claude-powered agents.

## Key Implementation Details

The macro needs to:
1. Parse function signature and parameters
2. Auto-derive JSON Schema for input from function parameters
3. Generate SdkMcpTool struct with tool metadata
4. Generate ToolHandler impl that wraps the function body
5. Validate function parameters (must be JSON-serializable)
6. Handle error cases gracefully
7. Support doc attributes for tool descriptions

## Next Steps

1. ⏭️ Investigate existing crate structure and proc macro patterns
2. ⏭️ Design macro implementation approach
3. ⏭️ Implement macro expansion logic
4. ⏭️ Write integration tests
5. ⏭️ Verify compilation and functionality
6. ⏭️ Add documentation and examples
7. ⏭️ Final clippy check and commit

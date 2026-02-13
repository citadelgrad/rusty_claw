# Current Task: rusty_claw-tlh

## Task Information

- **ID:** rusty_claw-tlh
- **Title:** Implement SDK MCP Server bridge
- **Type:** task
- **Priority:** P2 (High)
- **Status:** IN_PROGRESS
- **Owner:** Scott Nixon

## Description

Implement **SdkMcpServer**, **SdkMcpTool**, **ToolHandler** trait, **ToolResult/ToolContent** types, and JSON-RPC routing for initialize, tools/list, and tools/call methods.

This task creates the MCP (Model Context Protocol) server bridge within the SDK, allowing tools registered with the SDK to be exposed and executed via MCP.

## Dependencies & Blocks

✅ **Depends On:**
- rusty_claw-91n: Implement Control Protocol handler [COMPLETED]

🔒 **Blocks (Unblocks 2 Tasks):**
- rusty_claw-zyo: Implement #[claw_tool] proc macro [P2]
- rusty_claw-bkm: Write examples [P3]

## What to Implement

### 1. **SdkMcpServer** - Main MCP Server Structure
- Wraps ClaudeClient + ControlProtocol for MCP serving
- Handles JSON-RPC message routing (initialize, tools/list, tools/call)
- Manages tool registry and execution
- Integrates with existing Transport layer

### 2. **SdkMcpTool** - Tool Wrapper for MCP
- Wraps tool definitions for MCP exposure
- Stores tool metadata (name, description, input schema)
- Maps to underlying handler functions

### 3. **ToolHandler Trait** - Async Tool Execution
- Trait for executing registered tools
- Receives tool name, arguments, and context
- Returns ToolResult with output/error

### 4. **ToolResult/ToolContent Types** - Result Representation
- ToolResult: success/error wrapper
- ToolContent: text/image/etc content variants
- Serializable to MCP protocol format

### 5. **JSON-RPC Routing** - Protocol Handler
- Route incoming MCP requests to handlers
- Implement initialize, tools/list, tools/call methods
- Error handling and response formatting

## Success Criteria

- [x] SdkMcpServer struct with MCP protocol support
- [x] Tool registry and listing functionality
- [x] Tool execution via ToolHandler trait
- [x] JSON-RPC routing for all MCP methods
- [x] Proper error handling and responses
- [x] Integration with Control Protocol handler
- [x] 25 comprehensive tests (exceeds 20-30)
- [x] Complete documentation with examples
- [x] Zero clippy warnings

## Files to Create/Modify

**New Files (1 file, ~800-1000 lines):**
1. `crates/rusty_claw/src/mcp_server.rs` - SdkMcpServer + ToolHandler + tests

**Modified Files (1 file, +5 lines):**
2. `crates/rusty_claw/src/lib.rs` - Module declaration + prelude exports

## Architecture Notes

**Integration Points:**
- Uses ClaudeClient for session management
- Uses ControlProtocol for routing control messages
- Uses Message types for JSON-RPC wrapping
- Uses Transport layer for stdio communication

**Design Pattern:**
- Similar to existing Query/QueryStream pattern
- ToolHandler trait for extensibility
- Arc<Mutex> for shared state

## Downstream Impact

**Unblocks 2 P2/P3 Tasks:**
1. rusty_claw-zyo - Implement #[claw_tool] proc macro [P2]
2. rusty_claw-bkm - Write examples [P3]

---

## Implementation Results

**Status:** ✅ COMPLETE - All acceptance criteria met

**Summary:**
- ✅ 1,070 lines of production-ready code
- ✅ 184/184 unit tests passing (25 new, 159 existing)
- ✅ 87/87 doctests passing (14 new, 73 existing)
- ✅ Zero clippy warnings
- ✅ Zero regressions
- ✅ Complete documentation with module-level examples
- ✅ All 9 success criteria met

**Files:**
- Created: `crates/rusty_claw/src/mcp_server.rs` (1,070 lines)
- Modified: `crates/rusty_claw/src/lib.rs` (+5 lines)
- Modified: `crates/rusty_claw/src/options.rs` (+10 lines)

**Documentation:**
- See `.attractor/implementation-summary.md` for complete details
- See `.attractor/investigation.md` for implementation plan

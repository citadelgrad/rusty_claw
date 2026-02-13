# Investigation: rusty_claw-pwc - Define shared types and message structs

**Date:** 2026-02-13
**Task ID:** rusty_claw-pwc
**Priority:** P1 (Critical)
**Status:** IN_PROGRESS

## Task Overview

Implement shared message types and structures that will be used throughout the SDK for parsing and handling messages from the Claude Code CLI.

**Key Types to Implement:**
1. Message enum (System, Assistant, User, Result)
2. ContentBlock enum (Text, ToolUse, ToolResult, Thinking)
3. SystemMessage, AssistantMessage, ResultMessage structs
4. StreamEvent struct
5. Supporting types: UsageInfo, ToolInfo, McpServerInfo, ApiMessage, UserMessage

## Current State

### Existing Files:
- `crates/rusty_claw/src/lib.rs` - Main library file with placeholder modules
- `crates/rusty_claw/src/error.rs` - Complete error hierarchy (✓ rusty_claw-9pf)

### Missing:
- No `messages.rs` module exists yet
- No message type definitions

## Required Changes

### 1. Create New File: `crates/rusty_claw/src/messages.rs`

This file will contain all message type definitions as specified in `docs/SPEC.md:173-273`.

#### Core Message Types (from SPEC.md:177-260):

**Message Enum** (tagged on `type` field):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    System(SystemMessage),
    Assistant(AssistantMessage),
    User(UserMessage),
    Result(ResultMessage),
}
```

**SystemMessage** (tagged on `subtype` field):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum SystemMessage {
    Init {
        session_id: String,
        tools: Vec<ToolInfo>,
        mcp_servers: Vec<McpServerInfo>,
        #[serde(flatten)]
        extra: serde_json::Value,
    },
    CompactBoundary,
}
```

**AssistantMessage**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub message: ApiMessage,
    #[serde(default)]
    pub parent_tool_use_id: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
}
```

**ContentBlock** (tagged on `type` field):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: serde_json::Value,
        #[serde(default)]
        is_error: bool,
    },
    Thinking {
        thinking: String,
    },
}
```

**ResultMessage** (tagged on `subtype` field):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum ResultMessage {
    Success {
        result: String,
        #[serde(default)]
        duration_ms: Option<u64>,
        #[serde(default)]
        num_turns: Option<u32>,
        #[serde(default)]
        session_id: Option<String>,
        #[serde(default)]
        total_cost_usd: Option<f64>,
        #[serde(default)]
        usage: Option<UsageInfo>,
    },
    Error {
        error: String,
        #[serde(flatten)]
        extra: serde_json::Value,
    },
    InputRequired,
}
```

**StreamEvent** (SPEC.md:267-273):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    pub event_type: String,
    pub data: serde_json::Value,
}
```

#### Supporting Types (inferred from usage):

**ApiMessage** - Anthropic Messages API message structure:
```rust
/// A message in the Anthropic Messages API format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiMessage {
    pub role: String,  // "assistant", "user"
    pub content: Vec<ContentBlock>,
}
```

**UserMessage** - User input message:
```rust
/// A user message (currently not detailed in SPEC, placeholder)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessage {
    pub message: ApiMessage,
}
```

**UsageInfo** - Token usage information:
```rust
/// Token usage information from the API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageInfo {
    pub input_tokens: u32,
    pub output_tokens: u32,
}
```

**ToolInfo** - Tool definition from init message:
```rust
/// Information about an available tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub input_schema: Option<serde_json::Value>,
}
```

**McpServerInfo** - MCP server information:
```rust
/// Information about an MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerInfo {
    pub name: String,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}
```

### 2. Modify `crates/rusty_claw/src/lib.rs`

**Add messages module** (after line 64):
```rust
/// Message types and structures
pub mod messages;
```

**Update prelude** (add to prelude module around line 72):
```rust
pub use crate::messages::{
    Message, SystemMessage, AssistantMessage, ResultMessage, UserMessage,
    ContentBlock, StreamEvent, UsageInfo, ToolInfo, McpServerInfo, ApiMessage,
};
```

## Dependencies

### Crates Required:
- ✅ `serde` - Already in workspace dependencies
- ✅ `serde_json` - Already in workspace dependencies
- ✅ `serde` derive feature - Already enabled

### Blocked By:
- ✅ rusty_claw-9pf (Define error hierarchy) - **COMPLETE**

### Blocks:
- ❌ rusty_claw-sna (Implement query() function) - P1
- ❌ rusty_claw-1ke (Add unit tests for message parsing and fixtures) - P2
- ❌ rusty_claw-dss (Implement ClaudeAgentOptions builder) - P2

## Testing Strategy

### Unit Tests (`tests/` in messages.rs):

1. **Serialization/Deserialization Tests:**
   - Test each Message variant (System, Assistant, User, Result)
   - Test each SystemMessage variant (Init, CompactBoundary)
   - Test each ContentBlock variant (Text, ToolUse, ToolResult, Thinking)
   - Test each ResultMessage variant (Success, Error, InputRequired)

2. **Serde Tagging Tests:**
   - Verify `type` field discrimination for Message
   - Verify `subtype` field discrimination for SystemMessage and ResultMessage
   - Verify `type` field discrimination for ContentBlock

3. **Optional Fields Tests:**
   - Test `#[serde(default)]` behavior for optional fields
   - Test `#[serde(flatten)]` behavior for extra fields

4. **JSON Round-Trip Tests:**
   - Parse sample JSON → struct → serialize back to JSON
   - Verify structural equivalence

### Test Fixtures:

Create sample JSON files for common message patterns:
- Simple text response
- Tool use sequence
- System init message
- Error result
- Multi-turn conversation with thinking blocks

## Risks & Considerations

### Low Risk:
- ✅ Straightforward type definitions from SPEC
- ✅ All dependencies available
- ✅ Serde handles tagged enums elegantly
- ✅ Clear specification to follow

### Medium Risk:
- ⚠️ **Supporting types not fully specified in SPEC** - ApiMessage, UserMessage, UsageInfo, ToolInfo, McpServerInfo
  - **Mitigation:** Infer from Anthropic API docs and Python SDK structure
  - **Validation:** Will be tested in rusty_claw-1ke (message parsing fixtures)

### Assumptions:
1. ApiMessage follows standard Anthropic Messages API structure (role + content)
2. UsageInfo matches Anthropic API token usage structure
3. ToolInfo matches Claude Code CLI tool format
4. McpServerInfo is a simple name + extra fields structure

## Implementation Checklist

- [ ] Create `crates/rusty_claw/src/messages.rs`
- [ ] Implement Message enum with all variants
- [ ] Implement SystemMessage enum
- [ ] Implement AssistantMessage struct
- [ ] Implement ResultMessage enum
- [ ] Implement ContentBlock enum
- [ ] Implement StreamEvent struct
- [ ] Implement supporting types (ApiMessage, UserMessage, UsageInfo, ToolInfo, McpServerInfo)
- [ ] Add comprehensive documentation for all types
- [ ] Add module to lib.rs
- [ ] Add types to prelude
- [ ] Write unit tests for serialization/deserialization
- [ ] Write unit tests for serde tagging
- [ ] Write tests for optional fields handling
- [ ] Verify all tests pass with `cargo test`
- [ ] Run `cargo check` and `cargo clippy`

## Verification

### Build Verification:
```bash
cargo check --package rusty_claw
cargo clippy --package rusty_claw -- -D warnings
```

### Test Verification:
```bash
cargo test --package rusty_claw messages::tests
```

### Documentation Verification:
```bash
cargo doc --package rusty_claw --no-deps --open
```

## References

- **SPEC:** `docs/SPEC.md:173-273` - Message Types
- **Error Types:** `crates/rusty_claw/src/error.rs` - ClawError for error handling
- **Anthropic API:** Messages API structure (role, content, usage)
- **Python SDK:** `claude-agent-sdk-python` for reference patterns
- **Dependencies:** Workspace `Cargo.toml` for available crates

## Next Steps After Completion

1. Implement message parsing tests with fixtures (rusty_claw-1ke)
2. Use these types in Transport trait implementation (rusty_claw-6cn)
3. Use these types in query() function (rusty_claw-sna)
4. Build ClaudeAgentOptions using these types (rusty_claw-dss)

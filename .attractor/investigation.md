# Investigation: rusty_claw-bkm - Write Examples

**Task ID:** rusty_claw-bkm
**Status:** IN_PROGRESS
**Priority:** P3
**Date:** 2026-02-13

---

## Executive Summary

This task requires creating 4 comprehensive examples demonstrating the core usage patterns of the Rusty Claw SDK. All required APIs exist and are functional - this is purely an example-writing task.

**Task Scope:**
- Create 4 new example files (~510 lines total)
- Zero modifications to existing SDK code
- Follow established example pattern from subagent_usage.rs
- Each example must be self-contained, well-documented, and pass clippy

**Current State:** ✅ All APIs exist and are ready to use
- query() API is functional
- ClaudeClient is complete with all control operations
- #[claw_tool] proc macro is working
- Hook system is fully implemented

**What's Needed:** 📝 Example files only (no SDK changes)
1. simple_query.rs - Demonstrate one-shot query API
2. interactive_client.rs - Demonstrate ClaudeClient multi-turn sessions
3. custom_tool.rs - Demonstrate tool creation with #[claw_tool]
4. hooks_guardrails.rs - Demonstrate hook system for validation/monitoring

---

## Task Requirements

Create 4 working examples demonstrating core SDK usage patterns:

1. **simple_query.rs** - Basic SDK usage with simple queries
2. **interactive_client.rs** - Interactive multi-turn conversations using ClaudeClient
3. **custom_tool.rs** - Implementing and registering custom tools
4. **hooks_guardrails.rs** - Using hooks for guardrails and monitoring

**Quality Requirements:**
- Self-contained and runnable
- Comprehensive inline comments
- Module-level documentation with usage instructions
- Zero clippy warnings
- Follow existing example pattern (subagent_usage.rs)

---

## Dependencies

✅ All satisfied:
- rusty_claw-qrl (ClaudeClient) - CLOSED ✓
- rusty_claw-tlh (SDK MCP Server bridge) - CLOSED ✓

---

## Existing Examples

**Current examples:**
- `crates/rusty_claw/examples/subagent_usage.rs` (120 lines)

**Established Pattern:**
- Module-level documentation with `//!`
- Usage instructions in `# Usage` section
- Main async function with `#[tokio::main]`
- Detailed inline comments explaining each step
- Print statements showing configuration
- Commented-out code for real usage patterns

---

## API Investigation

### 1. Simple Query API

**File:** `crates/rusty_claw/src/query.rs`

**Core Function:**
```rust
pub async fn query(
    prompt: impl Into<String>,
    options: Option<ClaudeAgentOptions>,
) -> Result<impl Stream<Item = Result<Message, ClawError>>, ClawError>
```

**Key Features:**
- One-shot query to Claude
- Returns stream of `Result<Message, ClawError>`
- Automatically discovers and connects to Claude CLI
- Stream owns transport (CLI stays alive while consuming)

**Usage Pattern:**
```rust
use rusty_claw::query;
use rusty_claw::options::{ClaudeAgentOptions, PermissionMode};
use tokio_stream::StreamExt;

let options = ClaudeAgentOptions::builder()
    .max_turns(5)
    .permission_mode(PermissionMode::AcceptEdits)
    .build();

let mut stream = query("What files are in this directory?", Some(options)).await?;

while let Some(result) = stream.next().await {
    match result {
        Ok(Message::Assistant(msg)) => { /* handle */ },
        Ok(Message::Result(msg)) => { /* done */ },
        Err(e) => { /* error */ },
        _ => {}
    }
}
```

**Key Types:**
- `ClaudeAgentOptions` - Configuration (max_turns, permission_mode, model, etc.)
- `Message` - Response types (Assistant, Result, System, etc.)
- `QueryStream` - Stream wrapper that owns transport

---

### 2. ClaudeClient API

**File:** `crates/rusty_claw/src/client.rs`

**Lifecycle Methods:**
```rust
// Create client
ClaudeClient::new(options: ClaudeAgentOptions) -> Result<Self, ClawError>

// Connect and initialize session
connect() -> Result<(), ClawError>

// Send message and get response stream
send_message(content) -> Result<ResponseStream, ClawError>

// Close session gracefully
close() -> Result<(), ClawError>
```

**Control Operations:**
```rust
interrupt() -> Result<(), ClawError>                    // Cancel execution
set_permission_mode(mode) -> Result<(), ClawError>      // Change permissions
set_model(model) -> Result<(), ClawError>               // Switch model
mcp_status() -> Result<Value, ClawError>                // Query MCP status
rewind_files(message_id) -> Result<(), ClawError>       // Undo file changes
```

**Handler Registration:**
```rust
register_can_use_tool_handler(handler)  // Custom tool permissions
register_hook(hook_id, handler)         // Hook callbacks
register_mcp_message_handler(handler)   // MCP messages
```

**Usage Pattern:**
```rust
// Create and connect
let options = ClaudeAgentOptions::builder()
    .max_turns(10)
    .permission_mode(PermissionMode::AcceptEdits)
    .build();

let mut client = ClaudeClient::new(options)?;
client.connect().await?;

// Send message
let mut stream = client.send_message("What files are in this directory?").await?;

while let Some(result) = stream.next().await {
    match result {
        Ok(Message::Assistant(msg)) => { /* handle */ },
        Ok(Message::Result(_)) => break,
        Ok(_) => {},
        Err(e) => { /* error */ },
    }
}

// Control operations
client.interrupt().await?;
client.set_model("claude-sonnet-4-5").await?;
client.set_permission_mode(PermissionMode::Ask).await?;

// Close
client.close().await?;
```

**Key Traits:**
- `CanUseToolHandler` - Permission checking
- `HookHandler` - Hook callback implementation
- `McpMessageHandler` - MCP message handling

---

### 3. Custom Tool API

**File:** `crates/rusty_claw_macros/src/lib.rs`

**Proc Macro Usage:**
```rust
#[claw_tool(name = "tool-name", description = "Tool description")]
async fn tool_function(param1: String, param2: i32, opt: Option<String>) -> ToolResult {
    // Tool logic
    ToolResult::text(format!("Result: {}", param1))
}

// Generated function returns SdkMcpTool
let tool = tool_function();
```

**Manual Tool Creation:**
```rust
use async_trait::async_trait;

struct CustomHandler;

#[async_trait]
impl ToolHandler for CustomHandler {
    async fn call(&self, args: Value) -> Result<ToolResult, ClawError> {
        let name = args["name"].as_str().unwrap_or("World");
        Ok(ToolResult::text(format!("Hello, {}!", name)))
    }
}

let tool = SdkMcpTool::new(
    "greet",
    "Greet someone by name",
    json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" }
        },
        "required": ["name"]
    }),
    Arc::new(CustomHandler)
);
```

**Server Registration:**
```rust
use rusty_claw::mcp_server::{SdkMcpServerImpl, SdkMcpServerRegistry};

// Create server
let mut server = SdkMcpServerImpl::new("my-tools", "1.0.0");
server.register_tool(tool1);
server.register_tool(tool2);

// Register with client (via registry)
let registry = Arc::new(SdkMcpServerRegistry::new());
registry.register_server("my-tools", server).await;

client.register_mcp_message_handler(registry).await;
```

**ToolResult API:**
```rust
ToolResult::text("response text")           // Text content
ToolResult::error("error message")          // Error result
ToolContent::image("base64", "image/png")   // Image content
```

**Supported Parameter Types:**
- `String`, `str` - JSON string
- `i32`, `i64`, `u32`, `u64`, `f32`, `f64` - JSON number
- `bool` - JSON boolean
- `Option<T>` - Optional parameter (not required)
- `Vec<T>` - JSON array

---

### 4. Hooks API

**File:** `crates/rusty_claw/src/options.rs`

**Hook Events:**
```rust
pub enum HookEvent {
    ToolUse,            // When a tool is used
    Start,              // When agent starts
    Stop,               // When agent stops
    SubagentStart,      // When subagent starts
    SubagentStop,       // When subagent stops
    PreCompact,         // Before compaction
    Notification,       // System notification
    PermissionRequest,  // Permission request
}
```

**Hook Matcher:**
```rust
pub struct HookMatcher {
    pub tool_name: Option<String>,  // e.g., Some("Bash"), None for all
}

// Helper constructors
HookMatcher::all()           // Match all tools
HookMatcher::tool("Bash")    // Match specific tool
```

**Hook Configuration:**
```rust
let mut hooks = HashMap::new();

// Match Bash tool use
hooks.insert(
    HookEvent::ToolUse,
    vec![HookMatcher::tool("Bash")]
);

// Match all tools for Start event
hooks.insert(
    HookEvent::Start,
    vec![HookMatcher::all()]
);

let options = ClaudeAgentOptions::builder()
    .hooks(hooks)
    .build();
```

**Hook Handler Implementation:**
```rust
use async_trait::async_trait;

struct GuardrailHook;

#[async_trait]
impl HookHandler for GuardrailHook {
    async fn call(&self, event: HookEvent, input: Value) -> Result<Value, ClawError> {
        // Validation logic
        let tool_name = input["tool_name"].as_str().unwrap_or("");
        let tool_input = &input["tool_input"];

        // Check if allowed
        if tool_name == "Bash" && tool_input["command"].as_str().unwrap_or("").contains("rm -rf") {
            return Ok(json!({"approved": false, "reason": "Dangerous command"}));
        }

        Ok(json!({"approved": true}))
    }
}

// Register hook
client.register_hook("guardrail".to_string(), Arc::new(GuardrailHook)).await;
```

---

## Implementation Plan

### Phase 1: simple_query.rs (30 min)

**Goal:** Demonstrate basic one-shot query API

**File:** `crates/rusty_claw/examples/simple_query.rs`

**Structure:**
```rust
//! Simple query example demonstrating basic SDK usage
//!
//! This example shows how to:
//! - Configure options with ClaudeAgentOptions::builder()
//! - Execute a one-shot query using query()
//! - Stream and handle response messages
//! - Process different message types
//!
//! # Usage
//!
//! ```bash
//! cargo run --example simple_query --package rusty_claw
//! ```

use rusty_claw::prelude::*;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Rusty Claw Simple Query Example ===\n");

    // Configure options
    let options = ClaudeAgentOptions::builder()
        .max_turns(5)
        .permission_mode(PermissionMode::AcceptEdits)
        .model("claude-sonnet-4-5".to_string())
        .build();

    println!("Options configured:");
    println!("  - Max turns: {:?}", options.max_turns);
    println!("  - Permission mode: {:?}", options.permission_mode);
    println!("  - Model: {:?}", options.model);
    println!();

    // Execute query
    println!("Sending query...\n");
    let mut stream = query("What files are in this directory?", Some(options)).await?;

    // Stream responses
    while let Some(result) = stream.next().await {
        match result {
            Ok(Message::Assistant(msg)) => {
                // Handle assistant text
                for block in msg.message.content {
                    if let ContentBlock::Text { text } = block {
                        println!("Claude: {}", text);
                    }
                }
            }
            Ok(Message::Result(msg)) => {
                println!("\nResult: {:?}", msg);
                break;
            }
            Ok(_) => {},
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
        }
    }

    Ok(())
}
```

**Estimated Lines:** ~100 lines

**Success Criteria:**
- ✅ Compiles without warnings
- ✅ Demonstrates query() API
- ✅ Shows ClaudeAgentOptions builder
- ✅ Handles all message types
- ✅ Comprehensive comments

---

### Phase 2: interactive_client.rs (40 min)

**Goal:** Demonstrate multi-turn conversation with ClaudeClient

**File:** `crates/rusty_claw/examples/interactive_client.rs`

**Key Demonstrations:**
1. ClaudeClient lifecycle (new → connect → send → close)
2. Streaming responses
3. Multi-turn conversation
4. Control operations (interrupt, set_model, set_permission_mode)
5. Error handling

**Estimated Lines:** ~150 lines

**Success Criteria:**
- ✅ Compiles without warnings
- ✅ Demonstrates complete client lifecycle
- ✅ Shows control operations
- ✅ Multi-turn conversation pattern
- ✅ Comprehensive comments

---

### Phase 3: custom_tool.rs (40 min)

**Goal:** Demonstrate tool creation with #[claw_tool] macro

**File:** `crates/rusty_claw/examples/custom_tool.rs`

**Tools to Demonstrate:**
1. **Calculator** - Simple math (add, multiply) with i32 params
2. **Formatter** - String manipulation with String and Option<String>
3. **Echo** - Echo back input with optional prefix

**Key Demonstrations:**
- #[claw_tool] macro usage
- Different parameter types (String, i32, Option<T>)
- ToolResult creation
- SdkMcpServerImpl setup
- Tool registration
- Server registration with client

**Estimated Lines:** ~120 lines

**Success Criteria:**
- ✅ Compiles without warnings
- ✅ Demonstrates #[claw_tool] macro
- ✅ Shows multiple parameter types
- ✅ Server registration pattern
- ✅ Comprehensive comments

---

### Phase 4: hooks_guardrails.rs (45 min)

**Goal:** Demonstrate hook system for guardrails and monitoring

**File:** `crates/rusty_claw/examples/hooks_guardrails.rs`

**Hooks to Demonstrate:**
1. **GuardrailHook** - Validate tool inputs, block dangerous commands
2. **LoggingHook** - Log all tool usage and track metrics
3. **RateLimitHook** - Enforce rate limits on tool calls

**Key Demonstrations:**
- HookHandler trait implementation
- HookEvent enum usage
- HookMatcher configuration
- Hook registration with ClaudeClient
- Validation logic patterns
- Logging/monitoring patterns

**Estimated Lines:** ~140 lines

**Success Criteria:**
- ✅ Compiles without warnings
- ✅ Demonstrates hook system
- ✅ Shows validation patterns
- ✅ Shows logging patterns
- ✅ Comprehensive comments

---

### Phase 5: Testing & Verification (20 min)

**Goal:** Ensure all examples compile and pass quality checks

**Tasks:**
1. Compile all examples: `cargo build --examples --package rusty_claw`
2. Run clippy: `cargo clippy --examples --package rusty_claw`
3. Check documentation: `cargo doc --open`
4. Manual review of comments and structure

**Success Criteria:**
- ✅ All 4 examples compile
- ✅ Zero clippy warnings
- ✅ Clear, comprehensive documentation
- ✅ Follows established pattern

---

## Files to Create

### New Files (4 files, ~510 lines total)

| File | Lines | Purpose | Time |
|------|-------|---------|------|
| `crates/rusty_claw/examples/simple_query.rs` | ~100 | One-shot query demo | 30 min |
| `crates/rusty_claw/examples/interactive_client.rs` | ~150 | Multi-turn session demo | 40 min |
| `crates/rusty_claw/examples/custom_tool.rs` | ~120 | Tool creation demo | 40 min |
| `crates/rusty_claw/examples/hooks_guardrails.rs` | ~140 | Hook system demo | 45 min |
| **Total** | **~510** | **All examples** | **155 min** |

### No Files Modified

This task only creates new examples - zero changes to SDK code.

---

## Risk Assessment

### Risk Level: 🟢 LOW

**Why:**
- All required APIs exist and are functional
- Simple file creation task
- Examples are isolated from SDK codebase
- Clear pattern to follow (subagent_usage.rs)
- No breaking changes possible

### Success Probability: 95% (Very High)

**Reasoning:**
1. All APIs are complete and tested
2. Clear example pattern exists
3. Well-defined requirements
4. No dependencies on external work
5. Straightforward implementation

---

## Time Estimate

| Phase | Duration | Task |
|-------|----------|------|
| 1 | 30 min | simple_query.rs |
| 2 | 40 min | interactive_client.rs |
| 3 | 40 min | custom_tool.rs |
| 4 | 45 min | hooks_guardrails.rs |
| 5 | 20 min | Testing & verification |
| **Total** | **2.9 hours** | **All 4 examples** |

---

## Acceptance Criteria

✅ **All 4 examples created:**

1. ✅ simple_query.rs - Basic SDK usage with query()
2. ✅ interactive_client.rs - Multi-turn conversations with ClaudeClient
3. ✅ custom_tool.rs - Tool creation with #[claw_tool]
4. ✅ hooks_guardrails.rs - Hook system for guardrails/monitoring

**Quality Criteria:**
- Self-contained and runnable
- Comprehensive inline comments
- Module-level documentation
- Zero clippy warnings
- Follows existing example pattern

---

## Key Patterns Demonstrated

### simple_query.rs
- query() function usage
- ClaudeAgentOptions builder pattern
- Message stream handling
- Error handling

### interactive_client.rs
- ClaudeClient lifecycle
- ResponseStream consumption
- Control operations
- Multi-turn conversation

### custom_tool.rs
- #[claw_tool] proc macro
- ToolHandler trait
- SdkMcpServerImpl setup
- Tool registration
- Parameter types

### hooks_guardrails.rs
- HookHandler trait
- HookEvent enum
- HookMatcher configuration
- Validation logic
- Logging/monitoring

---

## Summary

**Status:** ✅ Ready to implement
**Complexity:** 🟢 LOW - Example creation only
**Scope:** 4 new files (~510 lines)
**Dependencies:** All satisfied
**Risk:** Very low (isolated examples)
**Time:** 2.9 hours
**Confidence:** 95% - Very high

**Key Insight:** This is a documentation task, not an implementation task. All SDK APIs are complete and functional. We're just creating examples to show users how to use them.

---

**Investigation Status:** ✅ COMPLETE
**Ready to Proceed:** YES
**Blockers:** NONE
**Next Action:** Phase 1 - Create simple_query.rs

# Permission Management

This document describes the permission system in rusty_claw for controlling which tools a Claude agent can execute.

## How Permissions Are Evaluated

When Claude attempts to use a tool, rusty_claw evaluates the request through multiple layers in a fixed order. The first layer to produce a definitive decision wins.

```
                    Tool Use Request
                         |
                         v
              +---------------------+
              | 1. Explicit Deny    |----> disallowed_tools list
              |    (highest prio)   |      Match? -> DENY
              +---------------------+
                         |
                    no match
                         |
                         v
              +---------------------+
              | 2. Explicit Allow   |----> allowed_tools list
              |                     |      In list? -> ALLOW
              +---------------------+      Not in list (and list non-empty)?
                         |                     -> fall through to default policy
                    no match / empty list
                         |
                         v
              +---------------------+
              | 3. Hooks            |----> PreToolUse HookHandler callbacks
              |    (HookHandler)    |      Return PermissionDecision:
              +---------------------+        Allow -> ALLOW
                         |                   Deny  -> DENY
                    no hook decision          Ask   -> ASK (prompt user)
                         |
                         v
              +---------------------+
              | 4. canUseTool       |----> CanUseToolHandler trait
              |    callback         |      (DefaultPermissionHandler or custom)
              +---------------------+      Returns Ok(true) -> ALLOW
                         |                 Returns Ok(false) -> DENY
                    no handler registered
                         |
                         v
              +---------------------+
              | 5. Permission Mode  |----> PermissionMode fallback
              |    (default policy) |      (see table below)
              +---------------------+
```

The `DefaultPermissionHandler` combines steps 2, 4, and 5 into a single `CanUseToolHandler` implementation. When you register it via `register_can_use_tool_handler`, it evaluates deny lists, allow lists, and mode-based defaults internally.

## Permission Modes

The `PermissionMode` enum controls the default policy when no explicit allow/deny rules match.

| Mode | CLI Arg | Default Policy | Description |
|------|---------|---------------|-------------|
| `Default` | `"default"` | Allow | No auto-approvals; relies on CLI defaults |
| `AcceptEdits` | `"accept-edits"` | Allow | Auto-approve file edit operations |
| `BypassPermissions` | `"bypass-permissions"` | Allow | Skip all permission checks |
| `Plan` | `"plan"` | Allow | Planning mode -- no tool execution |
| `Allow` | `"allow"` | Allow | Allow all tools without prompting |
| `Ask` | `"ask"` | Deny | Prompt user for each tool use |
| `Deny` | `"deny"` | Deny | Deny all tools by default |
| `Custom` | `"custom"` | Deny | Require hook-based decision; deny if no hook responds |

**Legacy modes** (`Default`, `AcceptEdits`, `BypassPermissions`, `Plan`) default to allowing all tools for backward compatibility. These modes are passed to the CLI via the `--permission-mode` flag and are primarily interpreted by the Claude CLI itself.

**Policy modes** (`Allow`, `Ask`, `Deny`, `Custom`) control `DefaultPermissionHandler` behavior when no allow/deny list matches.

Source: `crates/rusty_claw/src/options.rs`

## Setting Permission Mode

Permission mode is set through `ClaudeAgentOptions` at client construction time:

```rust
use rusty_claw::prelude::*;

let options = ClaudeAgentOptions::builder()
    .permission_mode(PermissionMode::AcceptEdits)
    .build();

let client = ClaudeClient::new(options)?;
```

The mode is converted to a CLI argument via `to_cli_arg()` and passed as `--permission-mode=<value>` when spawning the Claude CLI process.

```rust
// Multiple options together
let options = ClaudeAgentOptions::builder()
    .permission_mode(PermissionMode::Deny)
    .allowed_tools(vec!["Read".to_string(), "Grep".to_string(), "Glob".to_string()])
    .disallowed_tools(vec!["Bash".to_string()])
    .max_turns(10)
    .build();
```

Source: `crates/rusty_claw/src/options.rs`

## Tool Restrictions

### allowed_tools

A `Vec<String>` of tool names the agent is permitted to use. When this list is non-empty, only listed tools can be used (subject to deny list and default policy).

```rust
let options = ClaudeAgentOptions::builder()
    .allowed_tools(vec!["Read".to_string(), "Bash".to_string()])
    .build();
```

Passed to the CLI as `--allowed-tools=Read,Bash`.

When `allowed_tools` is empty, it imposes no restrictions -- all tools are candidates for the default policy.

### disallowed_tools

A `Vec<String>` of tool names that are always blocked. The deny list has the highest priority; a tool in `disallowed_tools` is blocked even if it also appears in `allowed_tools`.

```rust
let options = ClaudeAgentOptions::builder()
    .disallowed_tools(vec!["Bash".to_string(), "Write".to_string()])
    .build();
```

Passed to the CLI as `--disallowed-tools=Bash,Write`.

### permission_prompt_tool_allowlist

A `Vec<String>` of tools that require explicit permission prompts. This list is separate from the allow/deny mechanism and controls which tools trigger interactive user confirmation.

```rust
let options = ClaudeAgentOptions::builder()
    .permission_prompt_tool_allowlist(vec!["Edit".to_string()])
    .build();
```

Source: `crates/rusty_claw/src/options.rs`

## DefaultPermissionHandler

The `DefaultPermissionHandler` is a built-in `CanUseToolHandler` implementation that combines deny lists, allow lists, and mode-based defaults into a single evaluation:

1. Check `disallowed_tools` -- if the tool is listed, deny (highest priority)
2. Check `allowed_tools` -- if the list is non-empty and the tool is listed, allow
3. Fall back to `PermissionMode` default policy

```rust
use rusty_claw::permissions::DefaultPermissionHandler;
use rusty_claw::options::PermissionMode;

// Read-only agent: deny everything except read tools
let handler = DefaultPermissionHandler::builder()
    .mode(PermissionMode::Deny)
    .allowed_tools(vec![
        "Read".to_string(),
        "Glob".to_string(),
        "Grep".to_string(),
    ])
    .build();

// Permissive agent: allow everything except dangerous tools
let handler = DefaultPermissionHandler::builder()
    .mode(PermissionMode::Allow)
    .disallowed_tools(vec![
        "Bash".to_string(),
        "Write".to_string(),
    ])
    .build();
```

**Deny beats allow.** If a tool appears in both `allowed_tools` and `disallowed_tools`, it is denied.

Register it with the client:

```rust
use std::sync::Arc;

let handler = DefaultPermissionHandler::builder()
    .mode(PermissionMode::Deny)
    .allowed_tools(vec!["Read".to_string()])
    .build();

client.register_can_use_tool_handler(Arc::new(handler)).await;
```

Source: `crates/rusty_claw/src/permissions/handler.rs`

## Custom CanUseToolHandler

For logic beyond allow/deny lists, implement the `CanUseToolHandler` trait directly:

```rust
use rusty_claw::prelude::*;
use async_trait::async_trait;
use serde_json::Value;

struct AllowReadOnlyTools;

#[async_trait]
impl CanUseToolHandler for AllowReadOnlyTools {
    async fn can_use_tool(
        &self,
        tool_name: &str,
        _tool_input: &Value,
    ) -> Result<bool, ClawError> {
        Ok(matches!(tool_name, "Read" | "Grep" | "Glob"))
    }
}

// Register with client
let handler = Arc::new(AllowReadOnlyTools);
client.register_can_use_tool_handler(handler).await;
```

The `can_use_tool` method receives both the tool name and the tool input JSON, allowing input-level inspection (e.g., blocking specific bash commands while allowing others).

Source: `crates/rusty_claw/src/control/handlers.rs`

## Hook-Based Permissions

Hooks provide event-driven permission control via the `HookHandler` trait. A `PreToolUse` hook can inspect tool calls and return permission decisions.

### PermissionDecision Enum

```rust
pub enum PermissionDecision {
    Allow,  // Allow the action to proceed
    Deny,   // Deny the action
    Ask,    // Prompt the user for permission
}
```

### HookResponse

Hooks return a `HookResponse` that can carry a permission decision, a reason, additional context for Claude, and optionally modified tool input:

```rust
// Convenience constructors
let response = HookResponse::allow("Safe operation");
let response = HookResponse::deny("Destructive command blocked");
let response = HookResponse::ask("Confirm this operation?");

// Builder pattern for full control
let response = HookResponse::default()
    .with_permission(PermissionDecision::Allow)
    .with_reason("Approved after validation")
    .with_context("Additional context injected into Claude's prompt")
    .with_continue(true)        // continue processing subsequent hooks
    .with_updated_input(json!({"command": "safe-version"}));
```

When `HookResponse::deny()` is used, `should_continue` defaults to `false`, stopping further hook processing. All other constructors default to `should_continue: true`.

### Implementing a Permission Hook

```rust
use rusty_claw::prelude::*;
use async_trait::async_trait;
use serde_json::{json, Value};

struct GuardrailHook;

#[async_trait]
impl HookHandler for GuardrailHook {
    async fn call(
        &self,
        _event: HookEvent,
        input: Value,
    ) -> Result<Value, ClawError> {
        let tool_name = input["tool_name"].as_str().unwrap_or("unknown");
        let tool_input = &input["tool_input"];

        if tool_name == "Bash" {
            if let Some(cmd) = tool_input["command"].as_str() {
                if cmd.contains("rm -rf") {
                    return Ok(json!({
                        "approved": false,
                        "reason": "Destructive command blocked"
                    }));
                }
            }
        }

        Ok(json!({"approved": true}))
    }
}
```

### HookCallback Trait (Alternative)

For hooks that need access to `HookInput` and `HookContext` types, implement `HookCallback` directly or use an async function:

```rust
use rusty_claw::prelude::*;

async fn my_permission_hook(
    input: HookInput,
    _tool_use_id: Option<&str>,
    _context: &HookContext,
) -> Result<HookResponse, ClawError> {
    if let Some(tool_name) = &input.tool_name {
        if tool_name == "Bash" {
            return Ok(HookResponse::deny("Bash not allowed"));
        }
    }
    Ok(HookResponse::allow("OK"))
}
```

### Registering Hooks

Hooks are registered with the client using a unique hook ID:

```rust
use std::sync::Arc;

let guardrail = Arc::new(GuardrailHook);
client.register_hook("guardrail".to_string(), guardrail).await;
```

Hook matchers control which events trigger which hooks. Configure them in `ClaudeAgentOptions`:

```rust
use std::collections::HashMap;

let mut hooks = HashMap::new();
hooks.insert(HookEvent::PreToolUse, vec![HookMatcher::tool("Bash")]);
hooks.insert(HookEvent::UserPromptSubmit, vec![HookMatcher::all()]);

let options = ClaudeAgentOptions::builder()
    .hooks(hooks)
    .build();
```

`HookMatcher::all()` matches every tool. `HookMatcher::tool("Bash")` matches only the named tool.

Source: `crates/rusty_claw/src/hooks/`

## Mode Details

### AcceptEdits

Auto-approves file edit operations. Other tools follow standard permission evaluation. Use this when the agent needs to write code without user confirmation for each edit.

```rust
let options = ClaudeAgentOptions::builder()
    .permission_mode(PermissionMode::AcceptEdits)
    .build();
```

### BypassPermissions

Skips all permission checks entirely. Every tool is allowed. Use this only in trusted, sandboxed environments where the agent has full authority.

```rust
let options = ClaudeAgentOptions::builder()
    .permission_mode(PermissionMode::BypassPermissions)
    .build();
```

### Plan

Planning mode. The agent produces a plan but does not execute tools. Tools are technically allowed at the `DefaultPermissionHandler` level (legacy behavior), but the CLI itself restricts execution.

```rust
let options = ClaudeAgentOptions::builder()
    .permission_mode(PermissionMode::Plan)
    .build();
```

## Related Docs

- [HOOKS.md](./HOOKS.md) -- Hook system architecture and event types
- [SPEC.md](./SPEC.md) -- Full SDK specification
- [QUICKSTART.md](./QUICKSTART.md) -- Getting started guide
- `crates/rusty_claw/examples/hooks_guardrails.rs` -- Working example of hooks and guardrails

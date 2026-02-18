# Hooks

Intercept and control agent behavior at key execution points with hooks.

Hooks let you intercept agent execution at key points to add validation, logging, security controls, or custom logic. With hooks, you can:

- **Block dangerous operations** before they execute, like destructive shell commands or unauthorized file access
- **Log and audit** every tool call for compliance, debugging, or analytics
- **Transform inputs and outputs** to sanitize data, inject credentials, or redirect file paths
- **Require human approval** for sensitive actions like database writes or API calls
- **Track session lifecycle** to manage state, clean up resources, or send notifications

A hook has two parts:

1. **The matcher configuration**: tells the SDK which event to hook into (like `PreToolUse`) and which tools to match
2. **The handler implementation**: the logic that runs when the hook fires

The following example blocks the agent from modifying `.env` files. First, define a `HookHandler` that checks the file path, then register it on the `ClaudeClient` so it runs before any Write or Edit tool call:

```rust
use async_trait::async_trait;
use rusty_claw::prelude::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

// Define a hook handler that checks tool inputs for .env file access
struct ProtectEnvFiles;

#[async_trait]
impl HookHandler for ProtectEnvFiles {
    async fn call(&self, event: HookEvent, input: Value) -> Result<Value, ClawError> {
        let file_path = input["tool_input"]["file_path"]
            .as_str()
            .unwrap_or("");
        let file_name = file_path.rsplit('/').next().unwrap_or("");

        // Block the operation if targeting a .env file
        if file_name == ".env" {
            return Ok(json!({
                "hookSpecificOutput": {
                    "permissionDecision": "deny",
                    "permissionDecisionReason": "Cannot modify .env files"
                }
            }));
        }

        // Return empty object to allow the operation
        Ok(json!({}))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure matchers: trigger on PreToolUse for Write and Edit tools
    let mut hooks = HashMap::new();
    hooks.insert(
        HookEvent::PreToolUse,
        vec![
            HookMatcher::tool("Write"),
            HookMatcher::tool("Edit"),
        ],
    );

    let options = ClaudeAgentOptions::builder()
        .hooks(hooks)
        .build();

    let client = ClaudeClient::new(options)?;

    // Register the handler
    let handler = Arc::new(ProtectEnvFiles);
    client.register_hook("protect_env".to_string(), handler).await;

    Ok(())
}
```

This is a `PreToolUse` hook. It runs before the tool executes and can block or allow operations based on your logic. The rest of this guide covers all available hooks, their configuration options, and patterns for common use cases.

## Table of Contents

- [Available Hooks](#available-hooks)
- [Common Use Cases](#common-use-cases)
- [Configure Hooks](#configure-hooks)
  - [Matchers](#matchers)
  - [Handler Trait](#handler-trait)
  - [Callback Trait](#callback-trait)
  - [Input Data](#input-data)
  - [Response Types](#response-types)
  - [Permission Decision Flow](#permission-decision-flow)
- [Patterns](#patterns)
  - [Block a Tool](#block-a-tool)
  - [Modify Tool Input](#modify-tool-input)
  - [Add Context](#add-context)
  - [Auto-Approve Specific Tools](#auto-approve-specific-tools)
  - [Chain Multiple Hooks](#chain-multiple-hooks)
- [Advanced](#advanced)
  - [Subagent Tracking](#subagent-tracking)
  - [Async Operations in Hooks](#async-operations-in-hooks)
  - [Stateful Hooks](#stateful-hooks)
- [Troubleshooting](#troubleshooting)
- [Related Documentation](#related-documentation)

## Available Hooks

The `HookEvent` enum defines all lifecycle events that can trigger hooks:

| Hook Event | What Triggers It | Example Use Case |
|---|---|---|
| `PreToolUse` | Tool call request (can block or modify) | Block dangerous shell commands |
| `PostToolUse` | Tool execution result | Log all file changes to audit trail |
| `PostToolUseFailure` | Tool execution failure | Handle or log tool errors |
| `UserPromptSubmit` | User prompt submission | Inject additional context into prompts |
| `Stop` | Agent execution stop | Save session state before exit |
| `SubagentStart` | Subagent initialization | Track parallel task spawning |
| `SubagentStop` | Subagent completion | Aggregate results from parallel tasks |
| `PreCompact` | Conversation compaction request | Archive full transcript before summarizing |
| `Notification` | Agent status messages | Forward status updates to monitoring |
| `PermissionRequest` | Permission dialog would be displayed | Custom permission handling |

```rust
use rusty_claw::prelude::*;

// All variants of HookEvent
let events = vec![
    HookEvent::PreToolUse,
    HookEvent::PostToolUse,
    HookEvent::PostToolUseFailure,
    HookEvent::UserPromptSubmit,
    HookEvent::Stop,
    HookEvent::SubagentStart,
    HookEvent::SubagentStop,
    HookEvent::PreCompact,
    HookEvent::Notification,
    HookEvent::PermissionRequest,
];
```

## Common Use Cases

Hooks are flexible enough to handle many different scenarios. Here are common patterns organized by category.

**Security**
- Block dangerous commands (like `rm -rf /`, destructive SQL)
- Validate file paths before write operations
- Enforce allowlists/blocklists for tool usage

**Logging**
- Create audit trails of all agent actions
- Track execution metrics and performance
- Debug agent behavior in development

**Tool Interception**
- Redirect file operations to sandboxed directories
- Inject environment variables or credentials
- Transform tool inputs or outputs

**Authorization**
- Implement role-based access control
- Require human approval for sensitive operations
- Rate limit specific tool usage

## Configure Hooks

Configuring hooks in rusty_claw involves two steps:

1. **Declare matchers** in `ClaudeAgentOptions` to specify which events and tools trigger hooks
2. **Register handlers** on `ClaudeClient` to provide the logic that runs when hooks fire

```rust
use rusty_claw::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

// Step 1: Configure matchers in options
let mut hooks = HashMap::new();
hooks.insert(
    HookEvent::PreToolUse,
    vec![HookMatcher::tool("Bash")],
);

let options = ClaudeAgentOptions::builder()
    .hooks(hooks)
    .build();

let client = ClaudeClient::new(options)?;

// Step 2: Register handlers on the client
let handler = Arc::new(MyGuardrailHook);
client.register_hook("guardrail".to_string(), handler).await;
```

The `hooks` option on `ClaudeAgentOptions` is a `HashMap<HookEvent, Vec<HookMatcher>>` where:
- **Keys** are `HookEvent` variants (e.g., `HookEvent::PreToolUse`, `HookEvent::Stop`)
- **Values** are vectors of `HookMatcher`, each specifying which tools trigger the hook

Your hook handlers receive event data as `serde_json::Value` and return a `Result<Value, ClawError>` that tells the agent how to proceed.

### Matchers

Use `HookMatcher` to filter which tools trigger your hooks:

```rust
use rusty_claw::prelude::*;

// Match all tools (no filter)
let matcher = HookMatcher::all();
assert!(matcher.matches("Bash"));
assert!(matcher.matches("Read"));
assert!(matcher.matches("Write"));

// Match a specific tool by exact name
let matcher = HookMatcher::tool("Bash");
assert!(matcher.matches("Bash"));
assert!(!matcher.matches("Read"));

// Equivalent to HookMatcher::all() -- tool_name is None
let matcher = HookMatcher { tool_name: None };

// Equivalent to HookMatcher::tool("Write") -- tool_name is Some
let matcher = HookMatcher { tool_name: Some("Write".to_string()) };
```

Use a specific tool name in the matcher whenever possible. A matcher created with `HookMatcher::all()` runs your hook for every tool call, while `HookMatcher::tool("Bash")` only runs for Bash commands. Note that matchers filter by **tool name only** -- to filter by file path or other arguments, check the input inside your handler.

Matchers only apply to tool-based hooks (`PreToolUse`, `PostToolUse`, `PostToolUseFailure`, `PermissionRequest`). For lifecycle hooks like `Stop`, `SubagentStart`, and `Notification`, matchers are ignored and the hook fires for all events of that type.

Built-in tool names include: `Bash`, `Read`, `Write`, `Edit`, `Glob`, `Grep`, `WebFetch`, `Task`, and others. MCP tools use the pattern `mcp__<server>__<action>`.

### Handler Trait

The `HookHandler` trait is the primary interface for implementing hook logic. It receives the `HookEvent` that triggered it and the event data as a `serde_json::Value`:

```rust
use async_trait::async_trait;
use rusty_claw::prelude::*;
use serde_json::{json, Value};

struct MyHook;

#[async_trait]
impl HookHandler for MyHook {
    async fn call(
        &self,
        event: HookEvent,
        input: Value,
    ) -> Result<Value, ClawError> {
        let tool_name = input["tool_name"].as_str().unwrap_or("unknown");
        println!("Hook fired for event {:?}, tool: {}", event, tool_name);

        // Return empty object to allow without changes
        Ok(json!({}))
    }
}
```

Register handlers on the `ClaudeClient` with a unique string ID:

```rust
use std::sync::Arc;

let handler = Arc::new(MyHook);
client.register_hook("my_hook".to_string(), handler).await;
```

### Callback Trait

rusty_claw also provides the `HookCallback` trait for a more structured, typed approach to hook logic. Unlike `HookHandler` which works with raw `serde_json::Value`, `HookCallback` uses dedicated types for input and output:

```rust
use async_trait::async_trait;
use rusty_claw::prelude::*;

struct TypedHook;

#[async_trait]
impl HookCallback for TypedHook {
    async fn call(
        &self,
        input: HookInput,
        tool_use_id: Option<&str>,
        context: &HookContext,
    ) -> Result<HookResponse, ClawError> {
        if let Some(tool_name) = &input.tool_name {
            if tool_name == "Bash" {
                return Ok(HookResponse::deny("Bash not allowed"));
            }
        }
        Ok(HookResponse::allow("OK"))
    }
}
```

The `HookCallback` trait has a blanket implementation for async functions, so you can use plain functions directly:

```rust
use rusty_claw::prelude::*;

async fn my_hook(
    input: HookInput,
    tool_use_id: Option<&str>,
    context: &HookContext,
) -> Result<HookResponse, ClawError> {
    if let Some(tool_name) = &input.tool_name {
        if tool_name == "Bash" {
            return Ok(HookResponse::deny("Bash not allowed"));
        }
    }
    Ok(HookResponse::allow("OK"))
}

// my_hook automatically implements HookCallback via blanket impl
```

Every `HookCallback` receives three arguments:

1. **`input: HookInput`** -- Event details (tool name, tool input, output, error, prompt, metadata)
2. **`tool_use_id: Option<&str>`** -- Correlate `PreToolUse` and `PostToolUse` events for the same tool call
3. **`context: &HookContext`** -- Session context (session ID, available tools, agents, MCP servers)

### Input Data

#### HookInput

The `HookInput` struct contains information about the event that triggered the hook. All fields are optional because different events populate different fields:

| Field | Type | Description | Relevant Events |
|---|---|---|---|
| `tool_name` | `Option<String>` | Name of the tool being called | PreToolUse, PostToolUse, PostToolUseFailure, PermissionRequest |
| `tool_input` | `Option<Value>` | Arguments passed to the tool | PreToolUse, PostToolUse, PostToolUseFailure, PermissionRequest |
| `tool_output` | `Option<Value>` | Result returned from tool execution | PostToolUse |
| `error` | `Option<String>` | Error message from tool failure | PostToolUseFailure |
| `prompt` | `Option<String>` | The user's prompt text | UserPromptSubmit |
| `metadata` | `Option<HashMap<String, Value>>` | Additional event-specific data | All events |

Helper constructors simplify creation for common scenarios:

```rust
use rusty_claw::prelude::*;
use serde_json::json;

// For PreToolUse events
let input = HookInput::tool_use("Bash", json!({"command": "ls -la"}));

// For PostToolUse events
let input = HookInput::tool_success("Bash", json!({"output": "file.txt"}));

// For PostToolUseFailure events
let input = HookInput::tool_failure("Bash", "Command not found");

// For UserPromptSubmit events
let input = HookInput::prompt("Analyze this codebase");
```

#### HookContext

The `HookContext` struct provides session-level information to your hook:

| Field | Type | Description |
|---|---|---|
| `session_id` | `Option<String>` | Current session identifier |
| `available_tools` | `Option<Vec<String>>` | Tools available in the session |
| `agents` | `Option<Vec<String>>` | Active subagents |
| `mcp_servers` | `Option<Vec<String>>` | Connected MCP servers |
| `metadata` | `Option<HashMap<String, Value>>` | Additional context data |

Use the builder pattern to construct context:

```rust
use rusty_claw::prelude::*;

let context = HookContext::with_session("session-abc123")
    .with_tools(vec!["Bash".to_string(), "Read".to_string(), "Write".to_string()])
    .with_agents(vec!["researcher".to_string()])
    .with_mcp_servers(vec!["playwright".to_string()]);
```

### Response Types

#### HookResponse

The `HookResponse` struct tells the SDK how to proceed after your hook runs. Return a default response to allow the operation without changes. To block, modify, or annotate the operation, use the provided constructors and builder methods.

| Field | Type | Default | Description |
|---|---|---|---|
| `permission_decision` | `Option<PermissionDecision>` | `None` | Whether to allow, deny, or ask for the operation |
| `permission_decision_reason` | `Option<String>` | `None` | Explanation for the decision (shown to user) |
| `additional_context` | `Option<String>` | `None` | Context injected into Claude's prompt |
| `should_continue` | `bool` | `true` | Whether to continue processing subsequent hooks |
| `updated_input` | `Option<Value>` | `None` | Modified tool input (replaces original) |

**Quick constructors:**

```rust
use rusty_claw::prelude::*;

// Allow with reason
let response = HookResponse::allow("Safe operation");

// Deny with reason (also sets should_continue to false)
let response = HookResponse::deny("Dangerous operation detected");

// Ask user for confirmation
let response = HookResponse::ask("This will delete files. Continue?");
```

**Builder pattern** for more complex responses:

```rust
use rusty_claw::prelude::*;
use serde_json::json;

let response = HookResponse::default()
    .with_permission(PermissionDecision::Allow)
    .with_reason("Approved after validation")
    .with_context("User has admin privileges for this directory")
    .with_continue(true)
    .with_updated_input(json!({"command": "ls -la --color=never"}));
```

#### PermissionDecision

The `PermissionDecision` enum controls whether the operation proceeds:

| Variant | Effect |
|---|---|
| `PermissionDecision::Allow` | The operation proceeds without prompting the user |
| `PermissionDecision::Deny` | The operation is blocked |
| `PermissionDecision::Ask` | The user is prompted for confirmation |

```rust
use rusty_claw::prelude::*;

let allow = PermissionDecision::Allow;
let deny = PermissionDecision::Deny;
let ask = PermissionDecision::Ask;
```

### Permission Decision Flow

When multiple hooks or permission rules apply, the SDK evaluates them in this priority order:

1. **Deny** -- checked first. Any `Deny` result immediately blocks the operation.
2. **Ask** -- checked second. If no `Deny` but an `Ask` exists, the user is prompted.
3. **Allow** -- checked third. If all hooks return `Allow`, the operation proceeds.
4. **Default to Ask** -- if no hook returns a decision, the user is prompted.

If any hook returns `Deny`, the operation is blocked regardless of what other hooks return. A single `Deny` always overrides any number of `Allow` results.

```
  Hook A: Allow ─┐
  Hook B: Deny  ─┤──> Final Decision: Deny
  Hook C: Allow ─┘

  Hook A: Allow ─┐
  Hook B: Ask   ─┤──> Final Decision: Ask
  Hook C: Allow ─┘

  Hook A: Allow ─┐
  Hook B: Allow ─┤──> Final Decision: Allow
  Hook C: Allow ─┘
```

## Patterns

### Block a Tool

Return a deny decision to prevent tool execution. This is the most common security pattern:

```rust
use async_trait::async_trait;
use rusty_claw::prelude::*;
use serde_json::{json, Value};

struct BlockDangerousCommands;

#[async_trait]
impl HookHandler for BlockDangerousCommands {
    async fn call(&self, _event: HookEvent, input: Value) -> Result<Value, ClawError> {
        let command = input["tool_input"]["command"]
            .as_str()
            .unwrap_or("");

        let dangerous_patterns = [
            "rm -rf /",
            "rm -rf /*",
            "> /dev/sda",
            "dd if=/dev/zero",
            "mkfs.",
            ":(){ :|:& };:",
        ];

        for pattern in &dangerous_patterns {
            if command.contains(pattern) {
                return Ok(json!({
                    "hookSpecificOutput": {
                        "permissionDecision": "deny",
                        "permissionDecisionReason":
                            format!("Blocked dangerous command pattern: {}", pattern)
                    }
                }));
            }
        }

        Ok(json!({}))
    }
}
```

Using the typed `HookCallback` trait:

```rust
use rusty_claw::prelude::*;

async fn block_dangerous_commands(
    input: HookInput,
    _tool_use_id: Option<&str>,
    _context: &HookContext,
) -> Result<HookResponse, ClawError> {
    if let Some(tool_input) = &input.tool_input {
        if let Some(command) = tool_input.get("command").and_then(|v| v.as_str()) {
            if command.contains("rm -rf /") {
                return Ok(HookResponse::deny(
                    "Blocked dangerous command: rm -rf /",
                ));
            }
        }
    }
    Ok(HookResponse::allow("Command is safe"))
}
```

### Modify Tool Input

Return updated input to change what the tool receives. This is useful for sandboxing file operations or injecting default parameters:

```rust
use async_trait::async_trait;
use rusty_claw::prelude::*;
use serde_json::{json, Value};

struct RedirectToSandbox;

#[async_trait]
impl HookHandler for RedirectToSandbox {
    async fn call(&self, _event: HookEvent, input: Value) -> Result<Value, ClawError> {
        let tool_name = input["tool_name"].as_str().unwrap_or("");

        if tool_name == "Write" {
            let original_path = input["tool_input"]["file_path"]
                .as_str()
                .unwrap_or("");

            return Ok(json!({
                "hookSpecificOutput": {
                    "permissionDecision": "allow",
                    "updatedInput": {
                        "file_path": format!("/sandbox{}", original_path),
                        "content": input["tool_input"]["content"]
                    }
                }
            }));
        }

        Ok(json!({}))
    }
}
```

Using `HookResponse`:

```rust
use rusty_claw::prelude::*;
use serde_json::json;

async fn redirect_to_sandbox(
    input: HookInput,
    _tool_use_id: Option<&str>,
    _context: &HookContext,
) -> Result<HookResponse, ClawError> {
    if input.tool_name.as_deref() == Some("Write") {
        if let Some(tool_input) = &input.tool_input {
            let original_path = tool_input["file_path"]
                .as_str()
                .unwrap_or("");

            return Ok(HookResponse::allow("Redirected to sandbox")
                .with_updated_input(json!({
                    "file_path": format!("/sandbox{}", original_path),
                    "content": tool_input["content"]
                })));
        }
    }
    Ok(HookResponse::allow("No redirect needed"))
}
```

When using `updated_input`, you must also include a `permissionDecision` of `"allow"`. The updated input replaces the original tool input entirely, so include all required fields.

### Add Context

Inject additional information into the conversation that Claude can see. This is useful for providing guidance without blocking the operation:

```rust
use async_trait::async_trait;
use rusty_claw::prelude::*;
use serde_json::{json, Value};

struct SecurityReminder;

#[async_trait]
impl HookHandler for SecurityReminder {
    async fn call(&self, _event: HookEvent, input: Value) -> Result<Value, ClawError> {
        let tool_name = input["tool_name"].as_str().unwrap_or("");

        if tool_name == "Bash" {
            return Ok(json!({
                "systemMessage": "Remember: never run commands that modify \
                    system files outside the project directory."
            }));
        }

        Ok(json!({}))
    }
}
```

Using `HookResponse`:

```rust
use rusty_claw::prelude::*;

async fn add_project_context(
    input: HookInput,
    _tool_use_id: Option<&str>,
    context: &HookContext,
) -> Result<HookResponse, ClawError> {
    let tool_list = context
        .available_tools
        .as_ref()
        .map(|t| t.join(", "))
        .unwrap_or_else(|| "none".to_string());

    Ok(HookResponse::allow("Approved")
        .with_context(format!("Available tools: {}", tool_list)))
}
```

### Auto-Approve Specific Tools

Bypass permission prompts for trusted, read-only tools. This improves agent throughput for safe operations:

```rust
use async_trait::async_trait;
use rusty_claw::prelude::*;
use serde_json::{json, Value};

struct AutoApproveReadOnly;

#[async_trait]
impl HookHandler for AutoApproveReadOnly {
    async fn call(&self, _event: HookEvent, input: Value) -> Result<Value, ClawError> {
        let tool_name = input["tool_name"].as_str().unwrap_or("");

        let read_only_tools = ["Read", "Glob", "Grep"];

        if read_only_tools.contains(&tool_name) {
            return Ok(json!({
                "hookSpecificOutput": {
                    "permissionDecision": "allow",
                    "permissionDecisionReason": "Read-only tool auto-approved"
                }
            }));
        }

        Ok(json!({}))
    }
}
```

### Chain Multiple Hooks

Register multiple matchers for the same event to layer different responsibilities. Hooks execute in the order they appear in the vector. Keep each hook focused on a single responsibility:

```rust
use rusty_claw::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

let mut hooks = HashMap::new();

// Multiple matchers for PreToolUse, each targeting different tools
hooks.insert(
    HookEvent::PreToolUse,
    vec![
        HookMatcher::tool("Bash"),   // Trigger for Bash commands
        HookMatcher::tool("Write"),  // Trigger for file writes
        HookMatcher::tool("Edit"),   // Trigger for file edits
        HookMatcher::all(),          // Trigger for all tools (logging)
    ],
);

// Different matchers for different events
hooks.insert(
    HookEvent::PostToolUse,
    vec![HookMatcher::all()],  // Log all tool results
);

hooks.insert(
    HookEvent::Stop,
    vec![HookMatcher::all()],  // Cleanup on session end
);

let options = ClaudeAgentOptions::builder()
    .hooks(hooks)
    .build();

let client = ClaudeClient::new(options)?;

// Register handlers for different responsibilities
client.register_hook(
    "rate_limiter".to_string(),
    Arc::new(RateLimiterHook),
).await;

client.register_hook(
    "authorization".to_string(),
    Arc::new(AuthorizationHook),
).await;

client.register_hook(
    "input_sanitizer".to_string(),
    Arc::new(InputSanitizerHook),
).await;

client.register_hook(
    "audit_logger".to_string(),
    Arc::new(AuditLoggerHook),
).await;
```

## Advanced

### Subagent Tracking

Use `SubagentStart` and `SubagentStop` hooks to monitor subagent lifecycle. The `tool_use_id` parameter in `HookCallback` helps correlate parent agent calls with their subagents:

```rust
use async_trait::async_trait;
use rusty_claw::prelude::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

struct SubagentTracker {
    active: Arc<Mutex<HashMap<String, std::time::Instant>>>,
}

impl SubagentTracker {
    fn new() -> Self {
        Self {
            active: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl HookHandler for SubagentTracker {
    async fn call(&self, event: HookEvent, input: Value) -> Result<Value, ClawError> {
        match event {
            HookEvent::SubagentStart => {
                let agent_id = input["agent_id"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string();

                let mut active = self.active.lock().await;
                active.insert(agent_id.clone(), std::time::Instant::now());

                println!("[SUBAGENT] Started: {}", agent_id);
            }
            HookEvent::SubagentStop => {
                let agent_id = input["agent_id"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string();

                let mut active = self.active.lock().await;
                if let Some(start_time) = active.remove(&agent_id) {
                    let duration = start_time.elapsed();
                    println!(
                        "[SUBAGENT] Stopped: {} (duration: {:?})",
                        agent_id, duration
                    );
                }
            }
            _ => {}
        }

        Ok(json!({}))
    }
}

// Register for both subagent events
let mut hooks = HashMap::new();
hooks.insert(HookEvent::SubagentStart, vec![HookMatcher::all()]);
hooks.insert(HookEvent::SubagentStop, vec![HookMatcher::all()]);

let options = ClaudeAgentOptions::builder()
    .hooks(hooks)
    .build();

let client = ClaudeClient::new(options)?;
let tracker = Arc::new(SubagentTracker::new());
client.register_hook("subagent_tracker".to_string(), tracker).await;
```

### Async Operations in Hooks

Hooks can perform async operations like HTTP requests or database queries. Handle errors gracefully by catching them rather than propagating, so a failing webhook does not block the agent:

```rust
use async_trait::async_trait;
use rusty_claw::prelude::*;
use serde_json::{json, Value};

struct WebhookNotifier {
    webhook_url: String,
    client: reqwest::Client,
}

#[async_trait]
impl HookHandler for WebhookNotifier {
    async fn call(&self, event: HookEvent, input: Value) -> Result<Value, ClawError> {
        let tool_name = input["tool_name"]
            .as_str()
            .unwrap_or("unknown");

        // Fire-and-forget: do not block the agent on webhook delivery
        let result = self
            .client
            .post(&self.webhook_url)
            .json(&json!({
                "event": format!("{:?}", event),
                "tool": tool_name,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }))
            .send()
            .await;

        if let Err(e) = result {
            eprintln!("Webhook delivery failed (non-fatal): {}", e);
        }

        Ok(json!({}))
    }
}
```

For expensive operations that should not block the hook response, spawn a background task:

```rust
use async_trait::async_trait;
use rusty_claw::prelude::*;
use serde_json::{json, Value};

struct AsyncLogger;

#[async_trait]
impl HookHandler for AsyncLogger {
    async fn call(&self, event: HookEvent, input: Value) -> Result<Value, ClawError> {
        let event_desc = format!("{:?}", event);
        let input_clone = input.clone();

        // Spawn a background task so the hook returns immediately
        tokio::spawn(async move {
            // Expensive logging: write to database, send to analytics, etc.
            println!("[ASYNC LOG] {} - {}", event_desc, input_clone);
        });

        Ok(json!({}))
    }
}
```

### Stateful Hooks

Because `HookHandler` requires `Send + Sync`, use `Arc<Mutex<T>>` or `Arc<RwLock<T>>` to share mutable state across hook invocations:

```rust
use async_trait::async_trait;
use rusty_claw::prelude::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

struct RateLimiter {
    max_calls_per_minute: u32,
    call_log: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
}

impl RateLimiter {
    fn new(max_calls_per_minute: u32) -> Self {
        Self {
            max_calls_per_minute,
            call_log: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl HookHandler for RateLimiter {
    async fn call(&self, _event: HookEvent, input: Value) -> Result<Value, ClawError> {
        let tool_name = input["tool_name"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        let mut log = self.call_log.lock().await;
        let entries = log.entry(tool_name.clone()).or_default();

        // Remove entries older than 60 seconds
        let cutoff = Instant::now() - std::time::Duration::from_secs(60);
        entries.retain(|t| *t > cutoff);

        if entries.len() >= self.max_calls_per_minute as usize {
            return Ok(json!({
                "hookSpecificOutput": {
                    "permissionDecision": "deny",
                    "permissionDecisionReason": format!(
                        "Rate limit exceeded: {} calls/min for {}",
                        self.max_calls_per_minute, tool_name
                    )
                }
            }));
        }

        entries.push(Instant::now());
        Ok(json!({}))
    }
}
```

## Troubleshooting

### Hook not firing

- Verify the `HookEvent` variant matches the event you expect. Variants are case-sensitive in Rust (`HookEvent::PreToolUse`, not `HookEvent::pretooluse`).
- Check that your `HookMatcher` pattern matches the tool name. Use `HookMatcher::all()` temporarily to confirm the hook fires at all.
- Ensure the hook is under the correct event key in the `HashMap<HookEvent, Vec<HookMatcher>>`.
- For lifecycle hooks (`Stop`, `SubagentStart`, `SubagentStop`, `Notification`), matchers are ignored. These hooks fire for all events of that type.
- Hooks may not fire when the agent hits the `max_turns` limit because the session ends before hooks can execute.

### Matcher not filtering as expected

Matchers only match **tool names**, not file paths or other arguments. To filter by file path, check the input inside your handler:

```rust
use async_trait::async_trait;
use rusty_claw::prelude::*;
use serde_json::{json, Value};

struct MarkdownOnlyHook;

#[async_trait]
impl HookHandler for MarkdownOnlyHook {
    async fn call(&self, _event: HookEvent, input: Value) -> Result<Value, ClawError> {
        let file_path = input["tool_input"]["file_path"]
            .as_str()
            .unwrap_or("");

        // Skip non-markdown files
        if !file_path.ends_with(".md") {
            return Ok(json!({}));
        }

        // Process markdown files...
        Ok(json!({
            "hookSpecificOutput": {
                "permissionDecision": "ask",
                "permissionDecisionReason": "Confirm modification to documentation file"
            }
        }))
    }
}
```

### Tool blocked unexpectedly

- Check all `PreToolUse` hooks for `deny` returns. Add logging to see what `permissionDecisionReason` each hook returns.
- Remember that `HookMatcher::all()` (or `HookMatcher { tool_name: None }`) matches every tool. An overly broad matcher may be triggering a hook you did not intend.
- Review the [permission decision flow](#permission-decision-flow): a single `Deny` overrides any number of `Allow` results.

### Modified input not applied

- Ensure `updatedInput` is inside `hookSpecificOutput`, not at the top level of the returned JSON.
- You must also return `permissionDecision: "allow"` for the input modification to take effect.
- The `updatedInput` replaces the original tool input entirely. Include all required fields, not just the ones you want to change.

### Handler registration has no effect

- Handlers must be registered after the `ClaudeClient` is created but before sending queries. If the client is not connected, `register_hook` silently does nothing because the control protocol is not yet initialized.
- Ensure the hook ID string is unique. Registering a second handler with the same ID replaces the first.

### Compilation errors with HookCallback closures

The `HookCallback` blanket implementation requires specific lifetime bounds that can be complex with closures. If you encounter lifetime errors, prefer defining a named async function or implementing `HookCallback` on a struct directly instead of using a closure:

```rust
// Instead of a closure, use a named function:
async fn my_hook(
    input: HookInput,
    _tool_use_id: Option<&str>,
    _context: &HookContext,
) -> Result<HookResponse, ClawError> {
    Ok(HookResponse::allow("OK"))
}

// Or implement the trait on a struct:
struct MyHook;

#[async_trait]
impl HookCallback for MyHook {
    async fn call(
        &self,
        input: HookInput,
        _tool_use_id: Option<&str>,
        _context: &HookContext,
    ) -> Result<HookResponse, ClawError> {
        Ok(HookResponse::allow("OK"))
    }
}
```

## Related Documentation

- [Permissions](./PERMISSIONS.md) -- control what your agent can do
- [Technical Spec](./SPEC.md) -- system design and protocol docs
- [Example: hooks_guardrails.rs](../crates/rusty_claw/examples/hooks_guardrails.rs) -- complete working example
- [Source: hooks module](../crates/rusty_claw/src/hooks/mod.rs) -- hook types and traits
- [Source: options module](../crates/rusty_claw/src/options.rs) -- HookEvent and HookMatcher definitions
- [Source: control handlers](../crates/rusty_claw/src/control/handlers.rs) -- HookHandler trait and registration

# Subagents

Subagents allow a parent Claude agent to delegate specialized tasks to child agents, each with their own prompt, tool restrictions, and optional model override. This enables divide-and-conquer workflows where a coordinating agent breaks work into focused subtasks handled by purpose-built agents.

## Table of Contents

- [Overview](#overview)
- [Creating Subagents](#creating-subagents)
- [AgentDefinition Configuration](#agentdefinition-configuration)
- [Invoking Subagents](#invoking-subagents)
- [Detecting Subagent Messages](#detecting-subagent-messages)
- [Tool Restrictions](#tool-restrictions)
- [Lifecycle Hooks](#lifecycle-hooks)
- [Troubleshooting](#troubleshooting)
- [Related Docs](#related-docs)

## Overview

A subagent is a child Claude session spawned by the parent agent through the `Task` tool. Each subagent runs with its own system prompt and a restricted set of tools, isolated from the parent agent's conversation context. The parent receives the subagent's final result as a tool result in its own conversation.

Key properties of subagents:

- **Scoped tools** -- Each subagent operates with only the tools listed in its `AgentDefinition`. This prevents a documentation writer from executing shell commands, or a researcher from modifying files.
- **Prompt specialization** -- Each subagent receives a dedicated system prompt that focuses it on a specific role or task.
- **Model flexibility** -- Subagents can use a different model than the parent (e.g., a cheaper model for simple tasks).
- **No recursive spawning** -- Subagents cannot spawn their own subagents. The `Task` tool must not be included in a subagent's tool list.

## Creating Subagents

Subagents are defined using `AgentDefinition` and registered through the `agents` field on `ClaudeAgentOptions`.

```rust
use rusty_claw::options::AgentDefinition;
use rusty_claw::prelude::*;
use std::collections::HashMap;

let mut agents = HashMap::new();

agents.insert(
    "researcher".to_string(),
    AgentDefinition {
        description: "Research agent specialized in code analysis".to_string(),
        prompt: "You are a research assistant focused on analyzing code and finding patterns."
            .to_string(),
        tools: vec![
            "Read".to_string(),
            "Grep".to_string(),
            "Glob".to_string(),
        ],
        model: Some("claude-sonnet-4".to_string()),
    },
);

agents.insert(
    "writer".to_string(),
    AgentDefinition {
        description: "Writing agent specialized in documentation".to_string(),
        prompt: "You are a technical writer focused on creating clear documentation.".to_string(),
        tools: vec![
            "Read".to_string(),
            "Write".to_string(),
            "Edit".to_string(),
        ],
        model: None, // Inherits the parent's model
    },
);

let options = ClaudeAgentOptions::builder()
    .agents(agents)
    .allowed_tools(vec!["Task".to_string(), "Read".to_string(), "Bash".to_string()])
    .build();
```

The parent agent must include `"Task"` in its own `allowed_tools` for subagent invocation to work. Without it, the parent cannot spawn any subagents.

## AgentDefinition Configuration

The `AgentDefinition` struct defines everything the CLI needs to configure a subagent session:

```rust
pub struct AgentDefinition {
    pub description: String,
    pub prompt: String,
    pub tools: Vec<String>,
    pub model: Option<String>,
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `description` | `String` | Yes | Short summary of the agent's purpose. Shown to the parent agent so it knows when to delegate to this subagent. |
| `prompt` | `String` | Yes | System prompt for the subagent. Defines its role, behavior, and constraints. |
| `tools` | `Vec<String>` | Yes | Exhaustive list of tools the subagent may use. Must not include `"Task"`. |
| `model` | `Option<String>` | No | Model override. When `None`, the subagent inherits the parent session's model. |

## Invoking Subagents

The parent agent invokes subagents through the `Task` tool. There are two invocation patterns.

### Automatic invocation

When agents are registered, Claude sees their names and descriptions in its tool definitions. It will automatically choose the appropriate subagent based on the task at hand:

```
Parent: "Please analyze the codebase and then write documentation for the public API."

Claude delegates:
  1. Task(agent="researcher", prompt="Analyze public API surface in src/lib.rs...")
  2. Task(agent="writer", prompt="Write API documentation based on these findings...")
```

### Explicit invocation by name

You can instruct the parent agent to use a specific subagent in your prompt:

```rust
let options = ClaudeAgentOptions::builder()
    .agents(agents)
    .allowed_tools(vec!["Task".to_string()])
    .append_system_prompt(
        "Always delegate code searches to the 'researcher' agent.".to_string(),
    )
    .build();
```

The parent agent will see the registered agent names and can reference them by name in `Task` tool calls.

## Detecting Subagent Messages

When processing the message stream, messages originating from a subagent carry a `parent_tool_use_id` field in `AssistantMessage`. This field links the subagent's response back to the `Task` tool invocation that spawned it.

```rust
use rusty_claw::messages::{Message, AssistantMessage};

fn handle_message(msg: &Message) {
    if let Message::Assistant(assistant) = msg {
        match &assistant.parent_tool_use_id {
            Some(tool_use_id) => {
                // This message came from a subagent.
                // tool_use_id corresponds to the Task tool_use block
                // that spawned the subagent.
                println!(
                    "Subagent response (parent tool_use: {})",
                    tool_use_id
                );
            }
            None => {
                // This message came from the parent agent.
                println!("Parent agent response");
            }
        }
    }
}
```

The `AssistantMessage` struct:

```rust
pub struct AssistantMessage {
    pub message: ApiMessage,
    pub parent_tool_use_id: Option<String>,
    pub duration_ms: Option<u64>,
}
```

When `parent_tool_use_id` is `Some(id)`, the message belongs to a subagent session. When it is `None`, the message belongs to the parent agent.

## Tool Restrictions

Subagent tools should be the minimum set required for the agent's task. Below are common role-based combinations.

| Role | Tools | Notes |
|------|-------|-------|
| Researcher | `Read`, `Grep`, `Glob` | Read-only access for code analysis |
| Writer | `Read`, `Write`, `Edit` | File creation and modification |
| Executor | `Bash`, `Read` | Shell command execution with file reading |
| Reviewer | `Read`, `Grep`, `Glob`, `Bash` | Broad read access plus command execution for tests |
| Planner | `Read`, `Grep` | Minimal tools for planning without side effects |

Important constraints:

- **Never include `Task` in a subagent's tools.** Subagents cannot spawn their own subagents. Including `Task` will cause undefined behavior.
- **The parent must include `Task` in its `allowed_tools`.** Without it, the parent cannot invoke any subagents regardless of how many are registered.
- **Tool names are case-sensitive.** Use `"Read"`, not `"read"`.

```rust
// Correct: parent can invoke subagents, subagents cannot recurse
let parent_tools = vec!["Task".to_string(), "Read".to_string(), "Bash".to_string()];
let subagent_tools = vec!["Read".to_string(), "Grep".to_string()]; // No "Task"

// Wrong: subagent could try to spawn nested subagents
let bad_subagent_tools = vec!["Read".to_string(), "Task".to_string()]; // Do not do this
```

## Lifecycle Hooks

The `HookEvent` enum includes two events for tracking subagent lifecycles:

| Event | Triggered When | Typical Use |
|-------|---------------|-------------|
| `HookEvent::SubagentStart` | A subagent begins execution | Logging, metrics, initialization |
| `HookEvent::SubagentStop` | A subagent completes or terminates | Cleanup, metrics, post-processing |

### Registering lifecycle hooks

```rust
use rusty_claw::prelude::*;
use std::collections::HashMap;

let mut hooks = HashMap::new();

// Fire when any subagent starts
hooks.insert(
    HookEvent::SubagentStart,
    vec![HookMatcher::all()],
);

// Fire when any subagent stops
hooks.insert(
    HookEvent::SubagentStop,
    vec![HookMatcher::all()],
);

let options = ClaudeAgentOptions::builder()
    .agents(agents)
    .hooks(hooks)
    .allowed_tools(vec!["Task".to_string()])
    .build();
```

### Scoping hooks to specific tools

Use `HookMatcher::tool()` to restrict which tool triggers the hook:

```rust
// Only fire SubagentStart when matched against Bash tool usage
hooks.insert(
    HookEvent::SubagentStart,
    vec![HookMatcher::tool("Bash")],
);
```

See [HOOKS.md](./HOOKS.md) for full hook configuration details, handler examples, and best practices.

## Troubleshooting

### Subagent is never invoked

- Verify that `"Task"` is included in the parent agent's `allowed_tools`.
- Confirm the agent name in the `agents` HashMap matches what the parent references.
- Check that the agent's `description` clearly communicates when delegation is appropriate.

### Subagent fails with tool permission errors

- Ensure the subagent's `tools` list includes every tool it needs.
- Tool names are case-sensitive. `"bash"` will not match the built-in `"Bash"` tool.

### Subagent attempts to spawn nested subagents

- Remove `"Task"` from the subagent's `tools` list. Subagents must not recursively spawn agents.

### Messages are not attributed to the correct subagent

- Check `parent_tool_use_id` on `AssistantMessage`. Messages from the parent agent will have this field set to `None`.
- If multiple subagents run in sequence, each will have a distinct `parent_tool_use_id` corresponding to its `Task` tool invocation.

### Model override is not taking effect

- Verify the `model` field is set to `Some("model-name".to_string())` in the `AgentDefinition`.
- When `model` is `None`, the subagent inherits the parent's model setting.

## Related Docs

- [HOOKS.md](./HOOKS.md) -- Hook events and lifecycle callbacks
- [SPEC.md](./SPEC.md) -- Full SDK specification
- [QUICKSTART.md](./QUICKSTART.md) -- Getting started guide
- [Examples: subagent_usage.rs](../crates/rusty_claw/examples/subagent_usage.rs) -- Working example
- [Source: options.rs](../crates/rusty_claw/src/options.rs) -- `AgentDefinition` and `ClaudeAgentOptions`
- [Source: messages.rs](../crates/rusty_claw/src/messages.rs) -- `AssistantMessage` with `parent_tool_use_id`

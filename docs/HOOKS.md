# Hook Events

Hooks allow you to execute custom logic in response to specific lifecycle events in your Claude agent. This document covers all available hook events with special focus on subagent lifecycle hooks.

## Table of Contents

- [Overview](#overview)
- [Hook Event Types](#hook-event-types)
- [Subagent Lifecycle Hooks](#subagent-lifecycle-hooks)
  - [SubagentStart](#subagentstart)
  - [SubagentStop](#subagentStop)
- [Hook Configuration](#hook-configuration)
- [Examples](#examples)

## Overview

Hooks are registered in `ClaudeAgentOptions` using the `hooks` field. Each hook event can have multiple matchers that determine when the hook fires.

```rust
use rusty_claw::prelude::*;
use std::collections::HashMap;

let mut hooks = HashMap::new();
hooks.insert(
    HookEvent::SubagentStart,
    vec![HookMatcher { tool_name: Some("Bash".to_string()) }],
);

let options = ClaudeAgentOptions::builder()
    .hooks(hooks)
    .build();
```

## Hook Event Types

The `HookEvent` enum defines all available lifecycle events:

```rust
pub enum HookEvent {
    SubagentStart,      // When a subagent starts
    SubagentStop,       // When a subagent stops
    BeforeTurn,         // Before each agent turn
    AfterTurn,          // After each agent turn
    ToolUse,            // When a tool is about to be used
    // ... other events
}
```

## Subagent Lifecycle Hooks

### SubagentStart

**Triggered:** When a spawned subagent begins execution.

**Use Cases:**
- Logging subagent initialization
- Setting up monitoring or metrics
- Executing initialization scripts
- Notifying other services

**Hook Input:**

The hook receives contextual information about the starting subagent:

```json
{
  "agent_name": "researcher",
  "agent_id": "agent_abc123",
  "description": "Research agent specialized in code analysis",
  "tools": ["Read", "Grep", "Glob"],
  "model": "claude-sonnet-4"
}
```

**Example Configuration:**

```rust
use rusty_claw::prelude::*;
use rusty_claw::options::AgentDefinition;
use std::collections::HashMap;

let mut hooks = HashMap::new();

// Hook that matches Bash tool when subagent starts
hooks.insert(
    HookEvent::SubagentStart,
    vec![HookMatcher {
        tool_name: Some("Bash".to_string()),
    }],
);
```

**Example Hook Handler:**

```rust
use rusty_claw::hooks::{HookCallback, HookContext, HookResponse};

async fn handle_subagent_start(
    context: HookContext,
) -> Result<HookResponse, Box<dyn std::error::Error>> {
    println!("🚀 Subagent started: {}", context.agent_name);

    // Perform initialization logic
    // - Log to monitoring system
    // - Update dashboard
    // - Notify other services

    Ok(HookResponse::allow())
}
```

### SubagentStop

**Triggered:** When a subagent completes or terminates.

**Use Cases:**
- Cleanup resources
- Logging completion status
- Collecting metrics
- Triggering post-processing workflows

**Hook Input:**

The hook receives contextual information about the stopping subagent:

```json
{
  "agent_name": "researcher",
  "agent_id": "agent_abc123",
  "exit_code": 0,
  "duration_ms": 45230,
  "turns_completed": 12
}
```

**Example Configuration:**

```rust
use rusty_claw::prelude::*;
use std::collections::HashMap;

let mut hooks = HashMap::new();

// Hook that matches all tools when subagent stops
hooks.insert(
    HookEvent::SubagentStop,
    vec![HookMatcher {
        tool_name: None,  // Matches all tools
    }],
);
```

**Example Hook Handler:**

```rust
use rusty_claw::hooks::{HookCallback, HookContext, HookResponse};

async fn handle_subagent_stop(
    context: HookContext,
) -> Result<HookResponse, Box<dyn std::error::Error>> {
    println!(
        "✅ Subagent stopped: {} (exit code: {}, duration: {}ms)",
        context.agent_name,
        context.exit_code.unwrap_or(0),
        context.duration_ms.unwrap_or(0)
    );

    // Perform cleanup logic
    // - Close connections
    // - Save state
    // - Generate reports

    Ok(HookResponse::allow())
}
```

## Hook Configuration

### HookMatcher

The `HookMatcher` struct determines when a hook fires:

```rust
pub struct HookMatcher {
    /// Tool name pattern to match (e.g., "Bash", "mcp__*", or None for all)
    pub tool_name: Option<String>,
}
```

**Examples:**

```rust
// Match specific tool
HookMatcher {
    tool_name: Some("Bash".to_string()),
}

// Match all tools
HookMatcher {
    tool_name: None,
}

// Match tools with prefix (requires glob matching in handler)
HookMatcher {
    tool_name: Some("mcp__*".to_string()),
}
```

### Registering Multiple Matchers

You can register multiple matchers for the same event:

```rust
let mut hooks = HashMap::new();

hooks.insert(
    HookEvent::SubagentStart,
    vec![
        HookMatcher {
            tool_name: Some("Bash".to_string()),
        },
        HookMatcher {
            tool_name: Some("Read".to_string()),
        },
    ],
);
```

## Examples

### Complete Subagent Lifecycle Example

```rust
use rusty_claw::options::AgentDefinition;
use rusty_claw::prelude::*;
use std::collections::HashMap;

#[tokio::main]
async fn main() {
    // Define agents
    let mut agents = HashMap::new();
    agents.insert(
        "researcher".to_string(),
        AgentDefinition {
            description: "Research agent".to_string(),
            prompt: "You are a researcher".to_string(),
            tools: vec!["Read".to_string(), "Grep".to_string()],
            model: Some("claude-sonnet-4".to_string()),
        },
    );

    // Configure hooks
    let mut hooks = HashMap::new();

    // SubagentStart hook
    hooks.insert(
        HookEvent::SubagentStart,
        vec![HookMatcher {
            tool_name: Some("Bash".to_string()),
        }],
    );

    // SubagentStop hook
    hooks.insert(
        HookEvent::SubagentStop,
        vec![HookMatcher {
            tool_name: Some("Bash".to_string()),
        }],
    );

    // Build options
    let options = ClaudeAgentOptions::builder()
        .agents(agents)
        .hooks(hooks)
        .build();

    // Initialize client
    let transport = SubprocessCLITransport::default();
    let mut client = ClaudeClient::new(transport, options);
    client.initialize().await.unwrap();
}
```

### Monitoring Subagent Lifecycle

```rust
use rusty_claw::hooks::{HookCallback, HookContext, HookResponse};
use std::sync::Arc;
use tokio::sync::Mutex;

struct AgentMonitor {
    active_agents: Arc<Mutex<HashMap<String, std::time::Instant>>>,
}

impl AgentMonitor {
    fn new() -> Self {
        Self {
            active_agents: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn on_agent_start(&self, agent_name: String) {
        let mut active = self.active_agents.lock().await;
        active.insert(agent_name.clone(), std::time::Instant::now());
        println!("🚀 Agent started: {}", agent_name);
    }

    async fn on_agent_stop(&self, agent_name: String) {
        let mut active = self.active_agents.lock().await;
        if let Some(start_time) = active.remove(&agent_name) {
            let duration = start_time.elapsed();
            println!("✅ Agent stopped: {} (duration: {:?})", agent_name, duration);
        }
    }
}
```

## Best Practices

1. **Keep Hooks Fast**: Hook handlers block agent execution, so keep them lightweight.

2. **Use Async Operations**: For expensive operations, spawn tasks instead of blocking:
   ```rust
   tokio::spawn(async move {
       // Expensive operation
   });
   Ok(HookResponse::allow())
   ```

3. **Error Handling**: Always handle errors gracefully in hooks to avoid breaking agent execution.

4. **Resource Cleanup**: Use `SubagentStop` hooks to ensure proper resource cleanup.

5. **Logging**: Use hooks for comprehensive logging of agent lifecycle events.

## See Also

- [Examples: subagent_usage.rs](../crates/rusty_claw/examples/subagent_usage.rs)
- [SPEC.md: Hook Events](./SPEC.md)
- [options.rs: HookEvent](../crates/rusty_claw/src/options.rs)

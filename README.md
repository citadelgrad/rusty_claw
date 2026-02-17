# Rusty Claw

[![Crates.io](https://img.shields.io/crates/v/rusty_claw.svg)](https://crates.io/crates/rusty_claw)
[![Documentation](https://docs.rs/rusty_claw/badge.svg)](https://docs.rs/rusty_claw)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

> Rust implementation of the Claude Agent SDK

Build AI agents that autonomously read files, run commands, search the web, edit code, and more. Rusty Claw gives you the same tools, agent loop, and context management that power Claude Code, programmable in Rust.

```rust
use rusty_claw::prelude::*;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), ClawError> {
    let options = ClaudeAgentOptions::builder()
        .allowed_tools(vec!["Read".into(), "Edit".into(), "Bash".into()])
        .permission_mode(PermissionMode::AcceptEdits)
        .build();

    let mut stream = query("Find and fix the bug in auth.py", Some(options)).await?;

    while let Some(message) = stream.next().await {
        match message? {
            Message::Assistant(msg) => {
                for block in msg.message.content {
                    if let ContentBlock::Text { text } = block {
                        println!("{}", text);
                    }
                }
            }
            Message::Result(ResultMessage::Success { result, .. }) => {
                println!("Done: {}", result);
            }
            _ => {}
        }
    }

    Ok(())
}
```

Rusty Claw includes built-in tool support via the Claude CLI, so your agent can start working immediately without you implementing tool execution.

## Prerequisites

- **Rust** 1.70 or later
- **Claude Code CLI** v2.0.0 or later ([install guide](https://docs.anthropic.com/claude/docs/claude-cli))
- **Anthropic API key** set as `ANTHROPIC_API_KEY` environment variable

## Get Started

### 1. Add dependencies

```toml
[dependencies]
rusty_claw = "0.1"
tokio = { version = "1", features = ["full"] }
tokio-stream = "0.1"
```

### 2. Set your API key

```bash
export ANTHROPIC_API_KEY=your-api-key
```

### 3. Run your first agent

Create `src/main.rs`:

```rust
use rusty_claw::prelude::*;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), ClawError> {
    let options = ClaudeAgentOptions::builder()
        .allowed_tools(vec!["Bash".into(), "Glob".into()])
        .permission_mode(PermissionMode::AcceptEdits)
        .build();

    let mut stream = query("What files are in this directory?", Some(options)).await?;

    while let Some(message) = stream.next().await {
        if let Ok(Message::Result(ResultMessage::Success { result, .. })) = message {
            println!("{}", result);
        }
    }

    Ok(())
}
```

```bash
cargo run
```

**Ready to build?** Follow the [Quickstart](docs/QUICKSTART.md) to create an agent that finds and fixes bugs.

## Capabilities

### Simple Query API

One-shot queries with streaming responses via `query()`:

```rust
let mut stream = query("Explain this codebase", Some(options)).await?;
```

### Interactive Client

Persistent multi-turn sessions with `ClaudeClient`:

```rust
let mut client = ClaudeClient::new(options)?;
client.connect().await?;
let stream = client.send_message("Refactor the auth module").await?;
// ... later ...
let stream = client.send_message("Now add tests for it").await?;
client.close().await?;
```

### Custom Tools

Define MCP tools with the `#[claw_tool]` procedural macro:

```rust
#[claw_tool(name = "lookup_user", description = "Look up a user by ID")]
async fn lookup_user(user_id: String) -> ToolResult {
    ToolResult::text(format!("Found user: {user_id}"))
}
```

### Hooks and Guardrails

Lifecycle event hooks for validation, monitoring, and security:

```rust
// Block dangerous shell commands
async fn guard_bash(
    input: HookInput, _id: Option<&str>, _ctx: &HookContext,
) -> Result<HookResponse, ClawError> {
    if let Some(cmd) = input.tool_input.and_then(|v| v["command"].as_str().map(String::from)) {
        if cmd.contains("rm -rf") {
            return Ok(HookResponse::deny("Dangerous command blocked"));
        }
    }
    Ok(HookResponse::allow("OK"))
}
```

### Subagent Support

Spawn specialized agents with dedicated prompts and tool restrictions:

```rust
let mut agents = HashMap::new();
agents.insert("code-reviewer".into(), AgentDefinition {
    description: "Expert code reviewer for quality and security".into(),
    prompt: "Analyze code quality and suggest improvements.".into(),
    tools: vec!["Read".into(), "Grep".into(), "Glob".into()],
    model: Some("sonnet".into()),
});
```

### Type-Safe Messages

Strongly-typed message structures with serde for all Claude protocol interactions:

```rust
match message? {
    Message::System(SystemMessage::Init { session_id, tools, .. }) => { /* ... */ }
    Message::Assistant(msg) => { /* text, tool_use, thinking blocks */ }
    Message::Result(ResultMessage::Success { result, usage, .. }) => { /* ... */ }
    _ => {}
}
```

## Examples

The `examples/` directory contains runnable demonstrations:

| Example | Description |
|---------|-------------|
| [simple_query.rs](crates/rusty_claw/examples/simple_query.rs) | Basic SDK usage with `query()` |
| [interactive_client.rs](crates/rusty_claw/examples/interactive_client.rs) | Multi-turn conversations with `ClaudeClient` |
| [custom_tool.rs](crates/rusty_claw/examples/custom_tool.rs) | Creating custom tools with `#[claw_tool]` |
| [hooks_guardrails.rs](crates/rusty_claw/examples/hooks_guardrails.rs) | Implementing validation and monitoring hooks |
| [subagent_usage.rs](crates/rusty_claw/examples/subagent_usage.rs) | Spawning and managing subagents |

Run an example:

```bash
cargo run --example simple_query -p rusty_claw
```

Note: Examples that interact with Claude require a running Claude CLI with a valid API key.

## Documentation

| Guide | Description |
|-------|-------------|
| [Quickstart](docs/QUICKSTART.md) | Build a bug-fixing agent step by step |
| [Messages](docs/MESSAGES.md) | Message types and parsing reference |
| [Hooks](docs/HOOKS.md) | Intercept and control agent behavior |
| [Sessions](docs/SESSIONS.md) | Multi-turn sessions, resume, and fork |
| [MCP](docs/MCP.md) | Model Context Protocol tool integration |
| [Permissions](docs/PERMISSIONS.md) | Control what your agent can do |
| [Subagents](docs/SUBAGENTS.md) | Spawn specialized sub-agents |
| [Technical Spec](docs/SPEC.md) | Detailed architecture and protocol docs |

## Compared to Official SDKs

Rusty Claw is an **unofficial, community-driven** Rust SDK that provides the same core capabilities as Anthropic's official Python and TypeScript SDKs.

| | Rusty Claw (Rust) | Official (Python/TypeScript) |
|---|---|---|
| **API style** | `query()` + `ClaudeClient` | `query()` + streaming |
| **Message types** | Rust enums with serde | Dataclasses / interfaces |
| **Tool definition** | `#[claw_tool]` proc macro | Decorators / helper functions |
| **Hooks** | `HookCallback` trait + closures | Callback functions |
| **Async runtime** | tokio | asyncio / native async |
| **Error handling** | `Result<T, ClawError>` | Exceptions / thrown errors |
| **Performance** | Zero-cost abstractions, no GC | Interpreted runtime |

**When to use Rusty Claw:**
- Building Rust CLI tools, services, or system-level agents
- Performance-critical agent workloads where Python/Node overhead matters
- Embedding agent capabilities in existing Rust applications
- Platform teams building internal tools with Rust backends

## Architecture

```
User Application
    |
    +-- query() / ClaudeClient
    |       |
    +-- Control Protocol (JSON-RPC bidirectional)
    |       |
    +-- Transport Layer (trait: connect/read/write/close)
    |       |
    +-- SubprocessCLITransport (spawns claude CLI)
    |       |
    +-- SDK MCP Server Bridge (in-process tool hosting)
    |
    v
Claude Code CLI (>= 2.0.0)
```

Key components:
- **Transport Layer** - Bidirectional NDJSON communication over stdio
- **Control Protocol** - Handles permission requests, tool queries, and session management
- **MCP Integration** - Model Context Protocol server for exposing custom tools
- **Hook System** - Intercept and respond to lifecycle events
- **Type System** - Strongly-typed messages, content blocks, and protocol structures

## Development

```bash
cargo build              # Build
cargo test --workspace   # Run tests
cargo clippy --workspace # Lint
cargo doc --open         # Generate docs
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full development guide.

## License

Licensed under the MIT License. See [LICENSE](LICENSE) for details.

## Acknowledgments

Architecturally inspired by Anthropic's Python SDK ([claude-agent-sdk-python](https://github.com/anthropics/claude-agent-sdk-python)), licensed under MIT.

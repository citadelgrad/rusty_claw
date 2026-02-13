# Rusty Claw

[![Crates.io](https://img.shields.io/crates/v/rusty_claw.svg)](https://crates.io/crates/rusty_claw)
[![Documentation](https://docs.rs/rusty_claw/badge.svg)](https://docs.rs/rusty_claw)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

> Rust implementation of the Claude Agent SDK

Rusty Claw enables building Claude-powered agents in Rust with support for bidirectional JSONL transport, Claude Control Protocol (CCP) message handling, Model Context Protocol (MCP) tool integration, lifecycle hooks, and procedural macros for ergonomic tool definitions.

## Installation

Add `rusty_claw` to your `Cargo.toml`:

```toml
[dependencies]
rusty_claw = "0.1"
```

## Quick Start

```rust
use rusty_claw::prelude::*;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), ClawError> {
    let options = ClaudeAgentOptions::default();

    let mut stream = query("What is the capital of France?", options).await?;

    while let Some(message) = stream.next().await {
        match message {
            Message::Assistant(msg) => {
                for block in msg.content {
                    if let ContentBlock::Text { text } = block {
                        println!("{}", text);
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}
```

## Features

- **Simple Query API** - One-shot queries with streaming responses via `query()`
- **Interactive Client** - Persistent multi-turn sessions with `ClaudeClient`
- **Custom Tools** - Define MCP tools with the `#[claw_tool]` procedural macro
- **Hooks & Guardrails** - Lifecycle event hooks for validation and monitoring
- **Subagent Support** - Spawn specialized agents with dedicated prompts and tool restrictions
- **Type-Safe Messages** - Strongly-typed message structures for all Claude protocol interactions

## Examples

The `examples/` directory contains comprehensive demonstrations:

- **[simple_query.rs](crates/rusty_claw/examples/simple_query.rs)** - Basic SDK usage with the `query()` function
- **[interactive_client.rs](crates/rusty_claw/examples/interactive_client.rs)** - Multi-turn conversations with `ClaudeClient`
- **[custom_tool.rs](crates/rusty_claw/examples/custom_tool.rs)** - Creating custom tools with `#[claw_tool]`
- **[hooks_guardrails.rs](crates/rusty_claw/examples/hooks_guardrails.rs)** - Implementing validation and monitoring hooks
- **[subagent_usage.rs](crates/rusty_claw/examples/subagent_usage.rs)** - Spawning and managing subagents

Run an example:

```bash
cargo run --example simple_query
```

## Documentation

- **[API Documentation](https://docs.rs/rusty_claw)** - Complete API reference on docs.rs
- **[Hook System Guide](docs/HOOKS.md)** - Comprehensive guide to lifecycle hooks
- **[Technical Specification](docs/SPEC.md)** - Detailed architecture and protocol documentation

## Architecture

Rusty Claw implements the Claude Agent SDK protocol with the following key components:

- **Transport Layer** - Bidirectional JSONL communication over stdio with the Claude CLI
- **Control Protocol** - Handles permission requests, tool availability queries, and session management
- **MCP Integration** - Model Context Protocol server for exposing custom tools
- **Hook System** - Intercept and respond to lifecycle events (tool calls, permissions, subagents)
- **Type System** - Strongly-typed messages, content blocks, and protocol structures

## Requirements

- **Rust**: 1.70 or later
- **Claude CLI**: v2.0.0 or later (must be installed and available in PATH)

To install the Claude CLI, visit: https://docs.anthropic.com/claude/docs/claude-cli

## Development

Build the project:

```bash
cargo build
```

Run tests:

```bash
cargo test
```

Generate documentation:

```bash
cargo doc --open
```

## License

Licensed under the MIT License. See [LICENSE](LICENSE) for details.

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.

## Acknowledgments

Architecturally inspired by Anthropic's Python SDK ([claude-agent-sdk-python](https://github.com/anthropics/claude-agent-sdk-python)), licensed under MIT.

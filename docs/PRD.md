# Product Requirements Document: rusty_claw

> Rust implementation of the Claude Agent SDK

**Version:** 0.1.0-draft
**Date:** 2026-02-12
**Status:** Draft
**License:** MIT (derived from Anthropic's MIT-licensed Python SDK)

---

## 1. Overview

### 1.1 Problem Statement

The Claude Agent SDK enables developers to build autonomous AI agents powered by Claude Code. Official SDKs exist for Python and TypeScript, but the Rust ecosystem — widely used for CLI tools, systems programming, and performance-critical backends — has no equivalent. Developers building Rust applications that need agent capabilities must either shell out to the CLI manually, use FFI to call into the Python/TS SDKs, or build ad-hoc integrations.

### 1.2 Product Vision

**rusty_claw** is an idiomatic Rust SDK for building AI agents with Claude Code. It provides the same core capabilities as the official Python SDK (MIT-licensed reference implementation) while leveraging Rust's strengths: zero-cost abstractions, compile-time safety, fearless concurrency, and proc-macro ergonomics.

### 1.3 Target Users

| Persona | Use Case |
|---------|----------|
| **Rust CLI developers** | Build AI-powered CLI tools that edit files, run commands, analyze codebases |
| **Backend/infra engineers** | Embed agent capabilities in Rust services (code review bots, CI agents, deployment automation) |
| **Systems programmers** | Long-running agent processes where Python/Node overhead is unacceptable |
| **Platform teams** | Build internal developer tools with agent capabilities |

### 1.4 Non-Goals

- Re-implementing Claude's AI models or API (this is a client SDK)
- Providing a direct Anthropic API client (use `anthropic-rs` or raw HTTP for that)
- GUI or TUI — this is a library crate
- Supporting Claude Code CLI versions < 2.0.0

---

## 2. Legal & Licensing

### 2.1 Reference Implementation

The primary reference is the **Python SDK** ([claude-agent-sdk-python](https://github.com/anthropics/claude-agent-sdk-python)), which is published under the **MIT License** (Copyright 2025 Anthropic, PBC). The MIT license permits use, copy, modify, merge, publish, distribute, sublicense, and sell.

### 2.2 Licensing Strategy

- **rusty_claw** will be published under the **MIT License**
- The LICENSE file will credit Anthropic's Python SDK as the architectural reference
- No code will be copied from the TypeScript SDK (governed by Anthropic Commercial Terms)
- The project name avoids Anthropic trademarks — it is not "official" or "Anthropic-endorsed"

### 2.3 API Usage

Users of rusty_claw must independently comply with Anthropic's [Commercial Terms of Service](https://www.anthropic.com/legal/commercial-terms) when using the Claude API. The SDK itself does not access Anthropic's services — it communicates with the locally-installed Claude Code CLI.

---

## 3. Requirements

### 3.1 Core Capabilities

#### P0 — Must Have (v0.1.0)

| ID | Requirement | Description |
|----|------------|-------------|
| C-01 | **Simple query API** | `query()` async function for one-shot interactions. Returns a `Stream<Item = Message>`. |
| C-02 | **Message type system** | Strongly-typed message enums: `AssistantMessage`, `UserMessage`, `SystemMessage`, `ResultMessage`. Each with typed content blocks (`TextBlock`, `ToolUseBlock`, `ThinkingBlock`, `ToolResultBlock`). |
| C-03 | **Subprocess transport** | Spawn Claude Code CLI as a child process. Manage stdin/stdout/stderr. Parse newline-delimited JSON. Handle process lifecycle. |
| C-04 | **Configuration builder** | `ClaudeAgentOptions` builder with all supported options: `system_prompt`, `allowed_tools`, `permission_mode`, `max_turns`, `cwd`, `model`, `append_system_prompt`, `mcp_servers`, `output_format`. |
| C-05 | **Error types** | Typed error hierarchy: `ClawError` base, `CliNotFound`, `ConnectionError`, `ProcessError`, `JsonDecodeError`, `MessageParseError`. |
| C-06 | **CLI discovery** | Find Claude Code CLI binary in PATH and common install locations. Validate version >= 2.0.0. |

#### P1 — Should Have (v0.2.0)

| ID | Requirement | Description |
|----|------------|-------------|
| C-07 | **Bidirectional client** | `ClaudeClient` struct with connection lifecycle (`connect`, `query`, `interrupt`, `close`). Supports multi-turn conversations. |
| C-08 | **Control protocol** | Full bidirectional JSON-RPC control protocol over stdin/stdout. Send/receive control requests with unique IDs. Route incoming requests to handlers. |
| C-09 | **Hook system** | Register callbacks for `PreToolUse`, `PostToolUse`, `Stop`, `SubagentStop`, `Notification`, etc. Hooks can approve/deny tool use, add context, stop execution, or transform input. |
| C-10 | **Permission management** | `canUseTool` callback integration. Programmatic permission rule management (add, replace, remove). Mode switching (`default`, `acceptEdits`, `bypassPermissions`, `plan`). |
| C-11 | **Session management** | Resume previous sessions. Fork sessions. File checkpointing and `rewind_files()`. |

#### P2 — Nice to Have (v0.3.0)

| ID | Requirement | Description |
|----|------------|-------------|
| C-12 | **SDK MCP servers** | In-process MCP server bridge. Tool registration via proc macros. JSON-RPC routing for `initialize`, `tools/list`, `tools/call`. |
| C-13 | **Subagent definitions** | Define programmatic subagents with custom prompts, tools, and models. |
| C-14 | **Streaming events** | `include_partial_messages` support for real-time API stream events. |
| C-15 | **Structured output** | `output_format` with JSON schema validation for typed agent responses. |
| C-16 | **Sandbox configuration** | Programmatic sandbox settings for bash command isolation, network restrictions. |

### 3.2 Non-Functional Requirements

| ID | Requirement | Target |
|----|------------|--------|
| NF-01 | **Minimum Rust version** | 1.75+ (async trait stabilization) |
| NF-02 | **Async runtime** | `tokio` (with optional `async-std` support via feature flag) |
| NF-03 | **Compile time** | < 30s clean build on M-series Mac |
| NF-04 | **Dependencies** | Minimal — prefer `serde`, `tokio`, `thiserror`. Avoid heavy frameworks. |
| NF-05 | **Documentation** | Rustdoc on all public items. Examples for each major feature. |
| NF-06 | **Testing** | Unit tests for message parsing, integration tests against mock CLI process. |
| NF-07 | **Platform support** | macOS (primary), Linux (CI-verified), Windows (best-effort) |

### 3.3 API Ergonomics

| ID | Requirement | Description |
|----|------------|-------------|
| E-01 | **Builder pattern** | All options structs use the builder pattern (50+ fields need defaults). |
| E-02 | **Stream-based results** | All query results are `impl Stream<Item = Result<Message, ClawError>>`. |
| E-03 | **Context manager** | `ClaudeClient` implements `Drop` for cleanup. Provide `with_client()` helper for scoped usage. |
| E-04 | **Proc macro tools** | `#[claw_tool(name = "greet", description = "...")]` attribute macro for defining MCP tools. |
| E-05 | **Serde-native** | All types derive `Serialize`/`Deserialize`. Users can inspect raw JSON if needed. |

---

## 4. User Stories

### 4.1 Simple Query

```rust
use rusty_claw::query;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = query("What files are in this directory?", None).await?;

    while let Some(message) = stream.next().await {
        let msg = message?;
        println!("{msg}");
    }
    Ok(())
}
```

### 4.2 Interactive Client with Hooks

```rust
use rusty_claw::{ClaudeClient, ClaudeAgentOptions, HookEvent};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = ClaudeAgentOptions::builder()
        .allowed_tools(vec!["Read", "Write", "Bash"])
        .permission_mode("acceptEdits")
        .cwd("/my/project")
        .on_hook(HookEvent::PreToolUse, |input, _ctx| async move {
            if input.tool_name == "Bash" {
                let cmd = input.tool_input.get("command").unwrap_or_default();
                if cmd.contains("rm -rf") {
                    return HookResponse::deny("Dangerous command blocked");
                }
            }
            HookResponse::allow()
        })
        .build();

    let client = ClaudeClient::connect(options).await?;
    let response = client.query("Refactor the auth module").await?;

    // Process streaming response
    response.for_each(|msg| async { println!("{msg}") }).await;

    client.close().await?;
    Ok(())
}
```

### 4.3 Custom MCP Tool

```rust
use rusty_claw::{claw_tool, create_mcp_server, ClaudeAgentOptions, query};
use serde_json::json;

#[claw_tool(name = "lookup_user", description = "Look up a user by ID")]
async fn lookup_user(user_id: String) -> ToolResult {
    // Your custom logic
    let user = db::find_user(&user_id).await?;
    ToolResult::text(format!("User: {} ({})", user.name, user.email))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = create_mcp_server("my-tools", "1.0.0", vec![lookup_user()]);

    let options = ClaudeAgentOptions::builder()
        .sdk_mcp_servers(vec![server])
        .allowed_tools(vec!["mcp__my-tools__lookup_user"])
        .build();

    let mut stream = query("Look up user alice123", Some(options)).await?;
    // ...
    Ok(())
}
```

---

## 5. Success Criteria

| Metric | Target |
|--------|--------|
| All P0 requirements passing integration tests | v0.1.0 release gate |
| Published to crates.io | v0.1.0 milestone |
| Rustdoc coverage on public API | 100% |
| Can run the "simple query" example end-to-end | v0.1.0 smoke test |
| Can define and invoke a custom MCP tool | v0.3.0 smoke test |

---

## 6. Open Questions

| # | Question | Impact |
|---|----------|--------|
| 1 | Should we bundle the Claude Code CLI binary (like the Python SDK does) or require users to install it separately? | Distribution complexity vs. user experience |
| 2 | Should we support `async-std` via feature flags, or commit to `tokio` only? | Broader compatibility vs. maintenance cost |
| 3 | What minimum Claude Code CLI version should we target? The Python SDK requires >= 2.0.0. | Determines which control protocol features are available |
| 4 | Should the proc macro for tool definitions live in a separate `rusty_claw_macros` crate? | Standard Rust practice, but adds crate complexity |
| 5 | Should we provide a `no_std` transport trait for embedded use cases? | Niche but interesting for edge agent deployments |

# Examples

Runnable demos for the Rusty Claw SDK. Start with `simple_query` and work your way up.

## Prerequisites

Before running any example, make sure you have:

1. **Claude Code CLI** v2.0.0+ — the SDK spawns this as a subprocess

   ```bash
   claude --version   # should print 2.x.x or higher
   ```

   If this fails, [install the CLI](https://docs.anthropic.com/claude/docs/claude-cli) first.

2. **Anthropic API key** — the CLI needs this to talk to Claude

   ```bash
   export ANTHROPIC_API_KEY=sk-ant-...
   ```

3. **Verify the CLI works on its own** before trying the SDK:

   ```bash
   claude -p 'say hello in one word'
   ```

   If this doesn't produce a response, fix the CLI setup first — the SDK won't work either.

## How to Run

All examples run from the **repository root**:

```bash
cargo run -p examples --example simple_query
```

You can pass a **custom prompt** as an argument to any example:

```bash
cargo run -p examples --example simple_query -- "what is 2+2?"
```

If something goes wrong and the example exits silently, enable **debug logging** to see what the CLI wrote to stderr:

```bash
RUST_LOG=debug cargo run -p examples --example simple_query
```

Each example connects to the Claude CLI, sends a prompt, and prints streamed responses to stdout. They're non-interactive — just run and watch.

---

## `simple_query` — One-Shot Query

**Start here.** This is the minimal "hello world" for the SDK.

```bash
cargo run -p examples --example simple_query
cargo run -p examples --example simple_query -- "explain Rust lifetimes in one sentence"
```

**What it does:** Sends a prompt to Claude using the `query()` function and prints each streamed message as it arrives.

**What you'll see:**

```
Prompt:  "List the files in the current directory"
Connecting to Claude CLI...
Connected. Streaming response:

[session: sess_abc123...]
I'll list the files in the current directory.
[tool: Bash]
Here are the files:
- Cargo.toml
- README.md
- src/
...
---
Duration: 2340ms
Cost: $0.0031
```

**What to read in the code:**
- `ClaudeAgentOptions::builder()` — how to configure max turns, permission mode, model
- The `while let Some(result) = stream.next().await` loop — how to pattern-match on `Message` variants (`System`, `Assistant`, `Result`)
- `ContentBlock` matching — how text, tool calls, tool results, and thinking tokens each arrive as separate blocks

---

## `interactive_client` — Multi-Turn Client

**Next step.** Shows the persistent session API for when you need more than a single prompt.

```bash
cargo run -p examples --example interactive_client
cargo run -p examples --example interactive_client -- "what Rust edition are we using?"
```

**What it does:** Creates a `ClaudeClient`, connects to the CLI subprocess, sends a prompt, streams the response, then closes the session gracefully.

**What you'll see:**

```
Creating client...
Connecting to Claude CLI...
Connected.

Sending: "What is the current working directory?"
Streaming response:

I'll check the current working directory for you.
[calling tool: Bash with {"command":"pwd"}]
The current working directory is /path/to/rusty_claw
--- Done: Identified the current working directory
Session closed.
```

**What to read in the code:**
- `ClaudeClient::new(options)` + `connect()` — how to set up a persistent session
- `client.send_message(...)` — sends a user message and returns a `ResponseStream`
- `client.close()` — graceful shutdown (closes stdin, waits for the CLI to exit)
- The difference from `simple_query`: `ClaudeClient` keeps the CLI process alive between messages, so you could send follow-up prompts in a real app

---

## `custom_tool` — Custom MCP Tools

**Advanced.** Shows how to expose your own Rust functions as tools that Claude can call.

```bash
cargo run -p examples --example custom_tool
```

**What it does:** Defines two tools using the `#[claw_tool]` macro (`word_count` and `repeat`), registers them with an MCP server, then asks Claude to use them.

**What you'll see:**

```
Registering custom tools...
  - word_count(text: String)
  - repeat(message: String, times: Option<i32>)

Connecting to Claude CLI...
Connected.

Sending: "Use the word_count tool on 'hello world foo bar'..."
Streaming response:

[calling tool: word_count]
The word count for "hello world foo bar" is 4 words.

[calling tool: repeat]
hi
hi
hi

Done.
```

**What to read in the code:**
- `#[claw_tool(name = "...", description = "...")]` — the macro generates a handler struct and a builder function from a plain `async fn`
- `Option<i32>` parameters — become optional in the generated JSON Schema (not listed in `"required"`)
- `SdkMcpServerImpl` + `SdkMcpServerRegistry` — how to build the server and register it
- `client.register_mcp_message_handler(Arc::new(registry))` — wires the server into the client so incoming MCP calls get routed to your Rust functions

---

## Learning Path

After the three core examples above, explore these based on what you need:

### Configuration & Options

| Example | What you'll learn |
|---------|-------------------|
| `system_prompts` | Custom, preset, and appended system prompts |
| `agent_environment` | Working directory, env vars, CLI path |
| `advanced_config` | Settings sources, output format, beta features |
| `partial_messages` | Stream incremental content blocks for real-time UX |

### Permissions & Security

| Example | What you'll learn |
|---------|-------------------|
| `tool_permissions` | Static allow/deny lists, `DefaultPermissionHandler`, custom `CanUseToolHandler` |
| `hook_callbacks` | `HookCallback` trait, `HookInput`, `HookContext`, `HookResponse` builders |
| `hooks_guardrails` | `HookHandler` for validation, logging, and rate limiting |

### MCP Tools

| Example | What you'll learn |
|---------|-------------------|
| `advanced_tools` | `Vec<T>`, `bool` params, doc comments, name inference in `#[claw_tool]` |
| `image_tool_results` | `ToolContent::text()`, `ToolContent::image()`, multi-content results |
| `external_mcp` | External MCP server config (documents intended API — `McpServerConfig` is a stub) |

### Sessions & Runtime Control

| Example | What you'll learn |
|---------|-------------------|
| `session_resume` | Resume, fork, and name sessions |
| `file_checkpointing` | File snapshots, `rewind_files()` |
| `interrupt_and_status` | `interrupt()`, `mcp_status()` |

### Architecture & Internals

| Example | What you'll learn |
|---------|-------------------|
| `transport_layer` | `CliDiscovery`, `SubprocessCLITransport`, `Transport` trait |
| `rate_limit_handling` | `Message::RateLimitEvent`, `ClawError` variant matching |
| `subagent_usage` | `AgentDefinition`, subagent hooks |

### Offline Examples (no API key needed)

These examples only inspect types and configuration — they don't connect to Claude:

- `image_tool_results` — builds and serializes `ToolContent` and `ToolResult`
- `hook_callbacks` — tests `HookCallback` implementations locally
- `tool_permissions` — tests `DefaultPermissionHandler` decisions locally
- `transport_layer` — discovers the CLI binary (no connection)

---

## Troubleshooting

### "No messages received from Claude CLI"

The CLI started but exited without producing any JSON output. This usually means it wrote an error to stderr and quit. Run with logging to see what happened:

```bash
RUST_LOG=debug cargo run -p examples --example simple_query
```

Common causes:
- `ANTHROPIC_API_KEY` not set or invalid
- CLI not authenticated (`claude auth login`)
- Network issues reaching api.anthropic.com
- CLI version too old for `--output-format=stream-json`

### Other errors

**`ClawError::CliNotFound`** — The SDK can't find `claude` on your PATH. Make sure `claude --version` works.

**`ClawError::InvalidCliVersion`** — Your CLI is too old. Update to v2.0.0+.

**`cargo run` can't find the example** — Run from the repo root with `-p examples`:
```bash
# correct (from repo root)
cargo run -p examples --example simple_query

# wrong (from examples/ directory)
cargo run --example simple_query
```

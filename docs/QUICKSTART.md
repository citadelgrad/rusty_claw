# Quickstart

Get started with the Rusty Claw SDK to build AI agents in Rust that work autonomously.

---

Use Rusty Claw to build an AI agent that reads your code, finds bugs, and fixes them -- all without manual intervention. Rusty Claw wraps the Claude Code CLI, giving you a typed, async Rust interface to the full power of Claude's agentic capabilities.

**What you will do:**

1. Set up a Rust project with the Rusty Claw SDK
2. Create a file with some buggy code
3. Run an agent that finds and fixes the bugs automatically

## Prerequisites

- **Rust 1.70+** (install via [rustup](https://rustup.rs/))
- **Claude Code CLI >= 2.0.0** (install with `npm install -g @anthropic-ai/claude-code`)
- An **Anthropic API key** ([get one here](https://platform.claude.com/))

Verify your setup:

```bash
rustc --version      # Should be 1.70.0 or later
claude --version     # Should be 2.0.0 or later
```

## Setup

### 1. Create a project

```bash
cargo new my-agent && cd my-agent
```

For your own projects, you can run the SDK from any folder. The agent will have access to files in that directory and its subdirectories by default.

### 2. Add dependencies

Replace the contents of `Cargo.toml` with:

```toml
[package]
name = "my-agent"
version = "0.1.0"
edition = "2021"

[dependencies]
rusty_claw = "0.1"
tokio = { version = "1.35", features = ["full"] }
tokio-stream = "0.1"
```

### 3. Set your API key

Create a `.env` file (or `.envrc` if you use [direnv](https://direnv.net/)) in your project directory:

```bash
ANTHROPIC_API_KEY=your-api-key
```

Then export it in your shell:

```bash
export ANTHROPIC_API_KEY=your-api-key
```

The SDK also supports authentication via third-party API providers:

- **Amazon Bedrock**: set `CLAUDE_CODE_USE_BEDROCK=1` and configure AWS credentials
- **Google Vertex AI**: set `CLAUDE_CODE_USE_VERTEX=1` and configure Google Cloud credentials

## Create a buggy file

This quickstart walks you through building an agent that can find and fix bugs in code. First, you need a file with some intentional bugs for the agent to fix.

Create `utils.py` in the `my-agent` directory:

```python
def calculate_average(numbers):
    total = 0
    for num in numbers:
        total += num
    return total / len(numbers)


def get_user_name(user):
    return user["name"].upper()
```

This code has two bugs:

1. `calculate_average([])` crashes with division by zero
2. `get_user_name(None)` crashes with a TypeError

## Build an agent that finds and fixes bugs

Replace the contents of `src/main.rs` with:

```rust
use rusty_claw::prelude::*;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure the agent
    let options = ClaudeAgentOptions::builder()
        .allowed_tools(vec![
            "Read".to_string(),
            "Edit".to_string(),
            "Glob".to_string(),
        ])
        .permission_mode(PermissionMode::AcceptEdits)
        .build();

    // Start the agentic loop: streams messages as Claude works
    let mut stream = query(
        "Review utils.py for bugs that would cause crashes. Fix any issues you find.",
        Some(options),
    )
    .await?;

    // Process each message from the stream
    while let Some(result) = stream.next().await {
        match result {
            Ok(Message::Assistant(msg)) => {
                for block in &msg.message.content {
                    match block {
                        ContentBlock::Text { text } => {
                            println!("{}", text);
                        }
                        ContentBlock::ToolUse { name, .. } => {
                            println!("Tool: {}", name);
                        }
                        _ => {}
                    }
                }
            }
            Ok(Message::Result(result_msg)) => {
                match result_msg {
                    ResultMessage::Success { result, total_cost_usd, .. } => {
                        println!("\nDone: {}", result);
                        if let Some(cost) = total_cost_usd {
                            println!("Cost: ${:.4}", cost);
                        }
                    }
                    ResultMessage::Error { error, .. } => {
                        eprintln!("Error: {}", error);
                    }
                    ResultMessage::InputRequired => {
                        println!("Agent requires additional input.");
                    }
                }
            }
            Ok(_) => {} // System and User messages
            Err(e) => {
                eprintln!("Stream error: {}", e);
                break;
            }
        }
    }

    Ok(())
}
```

This code has three main parts:

1. **`query()`** -- the main entry point that creates the agentic loop. It returns a `Stream` of messages, so you use `while let Some(...)` with `StreamExt::next()` to consume messages as Claude works. The stream ends when Claude finishes the task or hits an error.

2. **`prompt`** -- what you want Claude to do. Claude figures out which tools to use based on the task.

3. **`ClaudeAgentOptions`** -- configuration for the agent. This example uses `allowed_tools` to restrict Claude to `Read`, `Edit`, and `Glob`, and `PermissionMode::AcceptEdits` to auto-approve file changes. Other options include `system_prompt`, `max_turns`, `model`, and more.

The `while let` loop keeps running as Claude thinks, calls tools, observes results, and decides what to do next. Each iteration yields a `Message`: Claude's reasoning, a tool call, a tool result, or the final outcome. The SDK handles the orchestration (tool execution, context management, retries) so you just consume the stream.

## Run your agent

```bash
cargo run
```

After running, check `utils.py`. You will see defensive code handling empty lists and null users. Your agent autonomously:

1. **Read** `utils.py` to understand the code
2. **Analyzed** the logic and identified edge cases that would crash
3. **Edited** the file to add proper error handling

This is what makes the Agent SDK different: Claude executes tools directly instead of asking you to implement them.

> If you see "Claude Code CLI not found", make sure the `claude` binary is in your PATH and version >= 2.0.0. Run `claude --version` to verify. If you see "API key not found", make sure you have set the `ANTHROPIC_API_KEY` environment variable.

## Key concepts

### Tools

Tools control what your agent can do:

| Tools | What the agent can do |
|-------|----------------------|
| `Read`, `Glob`, `Grep` | Read-only analysis |
| `Read`, `Edit`, `Glob` | Analyze and modify code |
| `Read`, `Edit`, `Bash`, `Glob`, `Grep` | Full automation |

### Permission modes

Permission modes control how much human oversight you want:

| Mode | Rust variant | Behavior |
|------|-------------|----------|
| `acceptEdits` | `PermissionMode::AcceptEdits` | Auto-approves file edits, asks for other actions |
| `bypassPermissions` | `PermissionMode::BypassPermissions` | Runs without prompts |
| `plan` | `PermissionMode::Plan` | Plan mode requiring approval before execution |
| `default` | `PermissionMode::Default` | Requires a permission handler callback |
| `allow` | `PermissionMode::Allow` | Allow all tool use without prompting |
| `ask` | `PermissionMode::Ask` | Prompt user for each tool use |
| `deny` | `PermissionMode::Deny` | Deny all tool use |
| `custom` | `PermissionMode::Custom` | Use custom permission logic via hooks |

The example above uses `AcceptEdits` mode, which auto-approves file operations so the agent can run without interactive prompts.

### Message types

The stream yields `Message` variants that represent everything Claude does:

| Variant | What it contains |
|---------|-----------------|
| `Message::System(SystemMessage)` | Session init with available tools, compact boundary events |
| `Message::Assistant(AssistantMessage)` | Claude's responses containing `ContentBlock` items |
| `Message::User(UserMessage)` | User input and tool results echoed back |
| `Message::Result(ResultMessage)` | Final outcome: `Success`, `Error`, or `InputRequired` |

Assistant messages contain `ContentBlock` items:

| Variant | What it contains |
|---------|-----------------|
| `ContentBlock::Text { text }` | Claude's reasoning and explanations |
| `ContentBlock::ToolUse { id, name, input }` | A tool invocation request |
| `ContentBlock::ToolResult { tool_use_id, content, is_error }` | Result from a tool execution |
| `ContentBlock::Thinking { thinking }` | Extended thinking tokens |

## Try other prompts

Now that your agent is set up, try changing the prompt string:

- `"Add docstrings to all functions in utils.py"`
- `"Add type hints to all functions in utils.py"`
- `"Create a README.md documenting the functions in utils.py"`

## Customize your agent

You can modify your agent's behavior by changing the options passed to the builder.

**Add web search capability:**

```rust
let options = ClaudeAgentOptions::builder()
    .allowed_tools(vec![
        "Read".to_string(),
        "Edit".to_string(),
        "Glob".to_string(),
        "WebSearch".to_string(),
    ])
    .permission_mode(PermissionMode::AcceptEdits)
    .build();
```

**Give Claude a custom system prompt:**

```rust
let options = ClaudeAgentOptions::builder()
    .allowed_tools(vec![
        "Read".to_string(),
        "Edit".to_string(),
        "Glob".to_string(),
    ])
    .permission_mode(PermissionMode::AcceptEdits)
    .system_prompt(SystemPrompt::Custom(
        "You are a senior Python developer. Always follow PEP 8 style guidelines.".to_string(),
    ))
    .build();
```

**Run commands in the terminal:**

```rust
let options = ClaudeAgentOptions::builder()
    .allowed_tools(vec![
        "Read".to_string(),
        "Edit".to_string(),
        "Glob".to_string(),
        "Bash".to_string(),
    ])
    .permission_mode(PermissionMode::AcceptEdits)
    .build();
```

With `Bash` enabled, try: `"Write unit tests for utils.py, run them, and fix any failures"`

**Limit conversation turns and select a model:**

```rust
let options = ClaudeAgentOptions::builder()
    .allowed_tools(vec!["Read".to_string(), "Edit".to_string(), "Glob".to_string()])
    .permission_mode(PermissionMode::AcceptEdits)
    .max_turns(10)
    .model("claude-sonnet-4")
    .build();
```

**Set a working directory:**

```rust
let options = ClaudeAgentOptions::builder()
    .allowed_tools(vec!["Read".to_string(), "Edit".to_string()])
    .permission_mode(PermissionMode::AcceptEdits)
    .cwd("/path/to/your/project")
    .build();
```

## Next steps

Now that you have created your first agent, learn how to extend its capabilities:

- **[Hooks](./HOOKS.md)** -- run custom code before or after tool calls, filter tool usage, and implement guardrails
- **[SPEC.md](./SPEC.md)** -- full SDK specification covering transport, control protocol, MCP integration, and architecture
- **[Examples](https://github.com/citadelgrad/rusty_claw/tree/main/crates/rusty_claw/examples)** -- working examples including simple queries, interactive clients, custom tools, subagents, and hook-based guardrails
- **[docs.rs](https://docs.rs/rusty_claw)** -- full API reference documentation

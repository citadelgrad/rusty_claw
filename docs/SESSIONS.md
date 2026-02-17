# Session Management

rusty_claw supports session persistence, resumption, forking, and file checkpointing through the `ClaudeAgentOptions` builder and the `ClaudeClient` multi-turn API.

## How Sessions Work

Every interaction with the Claude Code CLI occurs within a **session**. When a new session starts, the CLI assigns it a unique session ID (e.g., `sess_abc123`). This ID is delivered to your application in the very first message -- the system init message.

Sessions track conversation history, tool state, and MCP server connections. By capturing the session ID, you can resume or fork a session later without losing context.

### Session-Related Options

The `ClaudeAgentOptions` struct exposes four session-related fields:

| Field | Type | CLI Flag | Purpose |
|-------|------|----------|---------|
| `resume` | `Option<String>` | `--resume=ID` | Session ID to resume |
| `fork_session` | `bool` | `--fork-session` | Fork instead of resuming in-place |
| `session_name` | `Option<String>` | `--session-name=NAME` | Human-readable session name |
| `enable_file_checkpointing` | `bool` | `--enable-file-checkpointing` | Enable file-based checkpoints |

All four are set through the builder pattern on `ClaudeAgentOptions::builder()`.

## Getting the Session ID

The session ID is available in two message types: the system init message (always present) and the result success message (present on completion).

### From the System Init Message

The first message in every session is `Message::System(SystemMessage::Init { .. })`. Extract the session ID from it:

```rust
use rusty_claw::prelude::*;

fn capture_session_id(msg: &Message) -> Option<String> {
    match msg {
        Message::System(SystemMessage::Init { session_id, .. }) => {
            Some(session_id.clone())
        }
        _ => None,
    }
}
```

### From the Result Message

When the agent finishes, the result message may also carry the session ID:

```rust
use rusty_claw::prelude::*;

fn session_id_from_result(msg: &Message) -> Option<String> {
    match msg {
        Message::Result(ResultMessage::Success { session_id, .. }) => {
            session_id.clone()
        }
        _ => None,
    }
}
```

Note that `session_id` is `String` in `SystemMessage::Init` but `Option<String>` in `ResultMessage::Success`.

## Resuming Sessions

To resume a previous session, pass its ID to the `.resume()` builder method:

```rust
use rusty_claw::prelude::*;

let options = ClaudeAgentOptions::builder()
    .resume("sess_abc123")
    .build();
```

This generates the CLI argument `--resume=sess_abc123`. The CLI restores the full conversation history and tool state from that session.

### Resuming with a Named Session

You can combine `.resume()` with `.session_name()` to give the resumed session a human-readable label:

```rust
use rusty_claw::prelude::*;

let options = ClaudeAgentOptions::builder()
    .resume("sess_abc123")
    .session_name("debug-investigation")
    .build();
```

## Forking Sessions

Forking creates a **new** session that branches off from an existing one. The original session remains unchanged. This is useful when you want to explore an alternative direction without losing the original conversation.

```rust
use rusty_claw::prelude::*;

let options = ClaudeAgentOptions::builder()
    .resume("sess_abc123")
    .fork_session(true)
    .build();
```

### Resume vs. Fork

| Behavior | `.resume()` alone | `.resume()` + `.fork_session(true)` |
|----------|-------------------|-------------------------------------|
| Original session | Modified in-place | Preserved unchanged |
| New session ID | Same as original | New ID assigned |
| Conversation history | Continued | Copied then continued independently |
| Use case | Continue where you left off | Branch to explore alternatives |

Fork requires a session ID to fork from, so `.fork_session(true)` is always used together with `.resume()`.

## Multi-Turn Sessions with ClaudeClient

`ClaudeClient` manages a persistent connection to the Claude Code CLI subprocess. Its lifecycle is:

1. **Create** -- `ClaudeClient::new(options)` builds the client with configuration
2. **Connect** -- `client.connect().await` spawns the CLI process and initializes the session
3. **Interact** -- `client.send_message("...").await` sends queries and returns a `ResponseStream`
4. **Close** -- `client.close().await` gracefully shuts down the subprocess

### Capturing the Session ID in a Multi-Turn Flow

```rust
use rusty_claw::prelude::*;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = ClaudeAgentOptions::builder()
        .max_turns(10)
        .permission_mode(PermissionMode::AcceptEdits)
        .build();

    let mut client = ClaudeClient::new(options)?;
    client.connect().await?;

    let mut session_id: Option<String> = None;

    let mut stream = client.send_message("List the files in src/").await?;
    while let Some(result) = stream.next().await {
        match result {
            Ok(Message::System(SystemMessage::Init { session_id: sid, .. })) => {
                session_id = Some(sid);
            }
            Ok(Message::Assistant(msg)) => {
                for block in msg.message.content {
                    if let ContentBlock::Text { text } = block {
                        println!("{}", text);
                    }
                }
            }
            Ok(Message::Result(_)) => break,
            Ok(_) => {}
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    // Save session_id for later resumption
    if let Some(id) = &session_id {
        println!("Session ID: {}", id);
    }

    client.close().await?;
    Ok(())
}
```

### Resuming a Multi-Turn Session

```rust
use rusty_claw::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = ClaudeAgentOptions::builder()
        .resume("sess_abc123")
        .max_turns(10)
        .permission_mode(PermissionMode::AcceptEdits)
        .build();

    let mut client = ClaudeClient::new(options)?;
    client.connect().await?;

    // The session resumes with full conversation history intact.
    // Subsequent send_message() calls continue the existing conversation.
    let mut stream = client.send_message("What did we discuss earlier?").await?;
    // ... consume stream ...

    client.close().await?;
    Ok(())
}
```

## File Checkpointing

File checkpointing enables the CLI to save periodic snapshots of file state during a session. This allows rolling back file changes if an agent makes unwanted edits.

```rust
use rusty_claw::prelude::*;

let options = ClaudeAgentOptions::builder()
    .enable_file_checkpointing(true)
    .session_name("refactor-v2")
    .build();
```

This generates the CLI argument `--enable-file-checkpointing`. Checkpoints are managed by the CLI itself; rusty_claw passes the flag through without additional logic.

## Related Docs

- [SPEC.md](SPEC.md) -- Full SDK specification including transport and control protocol details
- [HOOKS.md](HOOKS.md) -- Hook system for intercepting tool use and agent lifecycle events
- [PRD.md](PRD.md) -- Product requirements document for rusty_claw

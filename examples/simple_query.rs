//! One-shot query — the simplest way to use rusty_claw.
//!
//! This example sends a single prompt to Claude and prints the streamed
//! response. It's the "hello world" of the SDK — start here to verify
//! your setup works, then look at `interactive_client` for multi-turn
//! sessions or `custom_tool` for defining your own tools.
//!
//! ## What happens under the hood
//!
//! 1. `query()` discovers the Claude CLI on your PATH, validates its version,
//!    and spawns it as a subprocess with `--output-format=stream-json`.
//! 2. The prompt is sent via CLI args (`-p "..."`), then stdin is closed
//!    (one-shot — no follow-up messages).
//! 3. The CLI streams NDJSON messages on stdout. Each line is parsed into a
//!    typed `Message` enum variant (System, Assistant, Result).
//! 4. When the stream ends, the CLI process exits and cleanup happens
//!    automatically.
//!
//! ## Run
//!
//! ```sh
//! # Default prompt
//! cargo run -p examples --example simple_query
//!
//! # Custom prompt
//! cargo run -p examples --example simple_query -- "what is 2+2?"
//!
//! # With debug logging (shows CLI stderr, transport details)
//! RUST_LOG=debug cargo run -p examples --example simple_query
//! ```

use std::io::{self, Write};

use rusty_claw::prelude::*;
use rusty_claw::query;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Enable tracing so SDK internals (CLI stderr, transport events) are visible.
    // Set RUST_LOG=debug to see what the CLI writes to stderr.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rusty_claw=warn".parse().unwrap()),
        )
        .with_target(false)
        .init();

    // Use a CLI argument as the prompt, or fall back to a default.
    let prompt = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "List the files in the current directory".to_string());

    // Configure the agent with builder pattern
    let options = ClaudeAgentOptions::builder()
        .max_turns(3)
        .model("claude-haiku-4-5")
        .permission_mode(PermissionMode::AcceptEdits)
        .build();

    println!("Prompt:  {prompt:?}");
    println!("Connecting to Claude CLI...");

    // Send a one-shot query and get a message stream.
    // This discovers the CLI, validates its version, spawns it, and sends the prompt.
    let mut stream = query(&prompt, Some(options)).await?;

    println!("Connected. Streaming response:\n");

    // Process each message from the stream.
    // Messages arrive as typed enum variants — match on the ones you care about.
    let mut message_count = 0;
    while let Some(result) = stream.next().await {
        message_count += 1;
        match result {
            Ok(Message::System(SystemMessage::Init { session_id, .. })) => {
                println!("[session: {}]", session_id);
            }
            Ok(Message::Assistant(msg)) => {
                for block in &msg.message.content {
                    match block {
                        ContentBlock::Text { text } => {
                            print!("{}", text);
                            io::stdout().flush()?;
                        }
                        ContentBlock::ToolUse { name, .. } => {
                            println!("\n[tool: {}]", name);
                        }
                        ContentBlock::ToolResult { is_error, .. } => {
                            if *is_error {
                                println!("[tool error]");
                            }
                        }
                        ContentBlock::Thinking { .. } => {
                            print!(".");
                            io::stdout().flush()?;
                        }
                    }
                }
            }
            Ok(Message::Result(ResultMessage::Success {
                duration_ms,
                total_cost_usd,
                ..
            })) => {
                println!("\n---");
                if let Some(ms) = duration_ms {
                    println!("Duration: {}ms", ms);
                }
                if let Some(cost) = total_cost_usd {
                    println!("Cost: ${:.4}", cost);
                }
            }
            Ok(Message::Result(ResultMessage::Error { error, .. })) => {
                eprintln!("Error: {}", error);
            }
            Ok(_) => {}
            Err(e) => eprintln!("Stream error: {}", e),
        }
    }

    // If we got zero messages, the CLI likely exited before producing output.
    // The most common cause is the CLI writing an error to stderr and exiting.
    // Run with RUST_LOG=debug to see CLI stderr output.
    if message_count == 0 {
        eprintln!();
        eprintln!("No messages received from Claude CLI.");
        eprintln!();
        eprintln!("The CLI probably wrote an error to stderr and exited.");
        eprintln!("Re-run with logging enabled to see what happened:");
        eprintln!();
        eprintln!("  RUST_LOG=debug cargo run -p examples --example simple_query");
        eprintln!();
        eprintln!("Common causes:");
        eprintln!("  - ANTHROPIC_API_KEY not set or invalid");
        eprintln!("  - CLI not authenticated (run: claude auth login)");
        eprintln!("  - Network issues reaching api.anthropic.com");
        eprintln!("  - CLI version incompatibility");
        eprintln!();
        eprintln!("You can also test the CLI directly:");
        eprintln!("  claude -p 'hello'");
    }

    Ok(())
}

//! Multi-turn interactive client — persistent sessions with `ClaudeClient`.
//!
//! Unlike `simple_query` (fire-and-forget), `ClaudeClient` keeps the CLI
//! process alive so you can send multiple messages in the same session.
//! This is what you'd use in a real application — a chatbot, an IDE
//! integration, a CI pipeline that asks Claude to review code, etc.
//!
//! ## What happens under the hood
//!
//! 1. `ClaudeClient::new()` stores your options but doesn't start anything.
//! 2. `connect()` spawns the CLI subprocess with `--input-format=stream-json`,
//!    initializes the control protocol, and starts a background task that
//!    routes incoming messages (separating control messages from user-facing
//!    ones).
//! 3. `send_message()` writes a JSON user message to the CLI's stdin and
//!    returns a `ResponseStream` you consume with `stream.next().await`.
//! 4. `close()` signals the CLI to shut down gracefully (closes stdin,
//!    waits for the process to exit).
//!
//! ## Run
//!
//! ```sh
//! # Default prompt
//! cargo run -p examples --example interactive_client
//!
//! # Custom prompt
//! cargo run -p examples --example interactive_client -- "explain ownership in Rust"
//!
//! # With debug logging (shows CLI stderr, control protocol)
//! RUST_LOG=debug cargo run -p examples --example interactive_client
//! ```

use std::io::{self, Write};

use rusty_claw::prelude::*;
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
        .unwrap_or_else(|| "What is the current working directory?".to_string());

    // Configure the session
    let options = ClaudeAgentOptions::builder()
        .max_turns(5)
        .model("claude-haiku-4-5")
        .permission_mode(PermissionMode::AcceptEdits)
        .build();

    // Create the client (no subprocess yet — just stores config)
    println!("Creating client...");
    let mut client = ClaudeClient::new(options)?;

    // Connect spawns the CLI subprocess, initializes the control protocol,
    // and starts a background message router.
    println!("Connecting to Claude CLI...");
    client.connect().await?;
    println!("Connected.\n");

    // Send a message and stream the response.
    // send_message() writes JSON to the CLI's stdin and returns a ResponseStream.
    println!("Sending: {prompt:?}");
    let mut stream = client.send_message(&prompt).await?;

    println!("Streaming response:\n");
    let mut message_count = 0;
    while let Some(result) = stream.next().await {
        message_count += 1;
        match result {
            Ok(Message::Assistant(msg)) => {
                for block in &msg.message.content {
                    match block {
                        ContentBlock::Text { text } => {
                            print!("{}", text);
                            io::stdout().flush()?;
                        }
                        ContentBlock::ToolUse { name, input, .. } => {
                            println!("\n[calling tool: {} with {:?}]", name, input);
                        }
                        ContentBlock::ToolResult {
                            content, is_error, ..
                        } => {
                            if *is_error {
                                println!("[tool error: {}]", content);
                            }
                        }
                        ContentBlock::Thinking { thinking } => {
                            println!("[thinking: {}...]", &thinking[..thinking.len().min(50)]);
                        }
                    }
                }
            }
            Ok(Message::Result(ResultMessage::Success { result, .. })) => {
                println!("\n--- Done: {}", result);
                break;
            }
            Ok(Message::Result(ResultMessage::Error { error, .. })) => {
                eprintln!("Error: {}", error);
                break;
            }
            Ok(Message::Result(ResultMessage::InputRequired)) => {
                println!("(Claude needs more input)");
                break;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("Stream error: {}", e);
                break;
            }
        }
    }

    if message_count == 0 {
        eprintln!();
        eprintln!("No messages received. Re-run with logging to diagnose:");
        eprintln!("  RUST_LOG=debug cargo run -p examples --example interactive_client");
    }

    // Close the session gracefully (closes stdin, waits for the CLI to exit)
    client.close().await?;
    println!("Session closed.");

    Ok(())
}

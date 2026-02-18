//! Rate limit handling — respond to rate limits and handle SDK errors.
//!
//! This example demonstrates:
//! - `Message::RateLimitEvent` — detecting rate limits in the message stream
//! - `ClawError` variants — pattern matching on all error types
//! - Graceful error recovery strategies
//!
//! ## Run
//! ```sh
//! cargo run -p examples --example rate_limit_handling
//! ```

use rusty_claw::prelude::*;
use std::io::{self, Write};
use tokio_stream::StreamExt;

/// Demonstrate pattern matching on all ClawError variants.
fn handle_error(err: &ClawError) {
    match err {
        ClawError::CliNotFound => {
            eprintln!("[ERROR] Claude CLI not found.");
            eprintln!("  Fix: npm install -g @anthropic-ai/claude-code");
        }
        ClawError::InvalidCliVersion { version } => {
            eprintln!("[ERROR] CLI version {} is too old.", version);
            eprintln!("  Fix: npm update -g @anthropic-ai/claude-code");
        }
        ClawError::Connection(msg) => {
            eprintln!("[ERROR] Connection failed: {}", msg);
            eprintln!("  Fix: Check that the CLI can start successfully.");
        }
        ClawError::Process { code, stderr } => {
            eprintln!("[ERROR] CLI exited with code {}: {}", code, stderr);
            eprintln!("  Fix: Check ANTHROPIC_API_KEY and network access.");
        }
        ClawError::JsonDecode(err) => {
            eprintln!("[ERROR] JSON parse error: {}", err);
            eprintln!("  This usually means the CLI sent unexpected output.");
        }
        ClawError::MessageParse { reason, raw } => {
            eprintln!("[ERROR] Message parse error: {}", reason);
            eprintln!("  Raw: {}...", &raw[..raw.len().min(100)]);
        }
        ClawError::ControlTimeout { subtype } => {
            eprintln!("[ERROR] Control timeout waiting for: {}", subtype);
            eprintln!("  Fix: Increase timeout or check for deadlocks.");
        }
        ClawError::ControlError(msg) => {
            eprintln!("[ERROR] Control protocol error: {}", msg);
        }
        ClawError::Io(err) => {
            eprintln!("[ERROR] I/O error: {}", err);
        }
        ClawError::ToolExecution(msg) => {
            eprintln!("[ERROR] Tool execution failed: {}", msg);
            eprintln!("  Fix: Check your tool handler implementation.");
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rusty_claw=warn".parse().unwrap()),
        )
        .with_target(false)
        .init();

    println!("=== Rate Limit & Error Handling ===\n");

    // --- 1. Error variant showcase ---
    println!("--- ClawError Variants ---\n");

    let errors: Vec<ClawError> = vec![
        ClawError::CliNotFound,
        ClawError::InvalidCliVersion {
            version: "1.5.2".to_string(),
        },
        ClawError::Connection("timeout after 30s".to_string()),
        ClawError::Process {
            code: 1,
            stderr: "API key invalid".to_string(),
        },
        ClawError::ControlTimeout {
            subtype: "prompt_response".to_string(),
        },
        ClawError::ControlError("permission denied".to_string()),
        ClawError::ToolExecution("handler panicked".to_string()),
    ];

    for err in &errors {
        handle_error(err);
        println!();
    }

    // --- 2. Live stream with rate limit awareness ---
    println!("--- Live Stream (rate limit aware) ---\n");

    let options = ClaudeAgentOptions::builder()
        .max_turns(3)
        .model("claude-haiku-4-5")
        .permission_mode(PermissionMode::AcceptEdits)
        .build();

    let mut client = ClaudeClient::new(options)?;
    client.connect().await?;

    let mut stream = client.send_message("Say hello in exactly 3 words.").await?;

    while let Some(result) = stream.next().await {
        match result {
            Ok(Message::RateLimitEvent(event)) => {
                // Rate limit events tell you about API throttling.
                // You can use these to implement backoff or notify users.
                println!("[RATE LIMIT] {:?}", event);
                println!("  Consider implementing exponential backoff.\n");
            }
            Ok(Message::Assistant(msg)) => {
                for block in &msg.message.content {
                    if let ContentBlock::Text { text } = block {
                        print!("{}", text);
                        io::stdout().flush()?;
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
                break;
            }
            Ok(Message::Result(ResultMessage::Error { error, .. })) => {
                eprintln!("\nCLI error: {}", error);
                break;
            }
            Err(e) => {
                eprintln!("\nStream error:");
                handle_error(&e);
                break;
            }
            _ => {}
        }
    }

    client.close().await?;
    println!("\nDone — demonstrated rate limit handling and error matching.");
    Ok(())
}

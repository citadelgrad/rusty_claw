//! Partial messages — stream incremental content as it arrives.
//!
//! When `.include_partial_messages(true)` is set, the CLI sends incomplete
//! content blocks as they're generated. This enables real-time display of
//! streaming text before the full message is assembled.
//!
//! Without partial messages, you only see complete `Assistant` messages
//! after the model finishes each turn. With partial messages, you see
//! incremental text deltas as they arrive.
//!
//! ## Run
//! ```sh
//! cargo run -p examples --example partial_messages
//! ```

use std::io::{self, Write};

use rusty_claw::prelude::*;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rusty_claw=warn".parse().unwrap()),
        )
        .with_target(false)
        .init();

    // Enable partial messages for real-time streaming
    let options = ClaudeAgentOptions::builder()
        .max_turns(3)
        .model("claude-haiku-4-5")
        .permission_mode(PermissionMode::AcceptEdits)
        .include_partial_messages(true)
        .build();

    println!(
        "include_partial_messages: {}",
        options.include_partial_messages
    );
    println!("Connecting to Claude CLI...\n");

    let mut client = ClaudeClient::new(options)?;
    client.connect().await?;

    let prompt = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Write a haiku about Rust programming.".to_string());

    println!("Prompt: {:?}\n", prompt);

    let mut stream = client.send_message(&prompt).await?;
    let mut message_count = 0;

    while let Some(result) = stream.next().await {
        message_count += 1;
        match result {
            Ok(Message::Assistant(msg)) => {
                // With partial messages enabled, you'll see many small Assistant
                // messages, each containing incremental text. Without it, you'd
                // see one large message at the end of each turn.
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
                println!("Messages received: {}", message_count);
                if let Some(ms) = duration_ms {
                    println!("Duration: {}ms", ms);
                }
                if let Some(cost) = total_cost_usd {
                    println!("Cost: ${:.4}", cost);
                }
                break;
            }
            Ok(Message::Result(ResultMessage::Error { error, .. })) => {
                eprintln!("Error: {}", error);
                break;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("Stream error: {}", e);
                break;
            }
        }
    }

    client.close().await?;
    println!("\nDone.");
    Ok(())
}

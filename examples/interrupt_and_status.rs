//! Interrupt and status — control a running session.
//!
//! This example demonstrates runtime control operations:
//! - `client.interrupt()` — cancel the current operation
//! - `client.mcp_status()` — query MCP server connection status
//!
//! These are useful for building interactive applications where users
//! might want to cancel long-running tasks or monitor server health.
//!
//! ## Run
//! ```sh
//! cargo run -p examples --example interrupt_and_status
//! ```

use rusty_claw::prelude::*;
use std::io::{self, Write};
use std::time::{Duration, Instant};
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

    println!("=== Interrupt and Status ===\n");

    let options = ClaudeAgentOptions::builder()
        .max_turns(5)
        .model("claude-haiku-4-5")
        .permission_mode(PermissionMode::AcceptEdits)
        .build();

    let mut client = ClaudeClient::new(options)?;
    client.connect().await?;

    // --- 1. MCP Status ---
    println!("--- MCP Status ---\n");
    match client.mcp_status().await {
        Ok(status) => {
            println!(
                "MCP status: {}",
                serde_json::to_string_pretty(&status).unwrap()
            );
        }
        Err(e) => {
            // mcp_status may not be supported in all CLI versions
            println!("MCP status unavailable: {}", e);
        }
    }

    // --- 2. Interrupt ---
    println!("\n--- Interrupt Demo ---\n");
    println!("Starting a long task, will interrupt after ~2 seconds of output...\n");

    let mut stream = client
        .send_message(
            "Write a very long, detailed essay about the history of programming languages, \
             covering at least 20 languages. Take your time and be thorough.",
        )
        .await?;

    let start = Instant::now();
    let interrupt_after = Duration::from_secs(2);
    let mut interrupted = false;
    let mut char_count = 0;

    while let Some(result) = stream.next().await {
        match result {
            Ok(Message::Assistant(msg)) => {
                for block in &msg.message.content {
                    if let ContentBlock::Text { text } = block {
                        print!("{}", text);
                        io::stdout().flush()?;
                        char_count += text.len();
                    }
                }

                // Interrupt after the timeout elapses
                if !interrupted && start.elapsed() > interrupt_after {
                    println!(
                        "\n\n[Sending interrupt signal after {:.1}s...]",
                        start.elapsed().as_secs_f64()
                    );
                    match client.interrupt().await {
                        Ok(_) => println!("[Interrupt sent successfully]"),
                        Err(e) => println!("[Interrupt failed: {}]", e),
                    }
                    interrupted = true;
                }
            }
            Ok(Message::Result(ResultMessage::Success {
                duration_ms,
                total_cost_usd,
                ..
            })) => {
                println!(
                    "\n\n--- Task completed{} ---",
                    if interrupted {
                        " (after interrupt)"
                    } else {
                        ""
                    }
                );
                println!("Characters received: {}", char_count);
                if let Some(ms) = duration_ms {
                    println!("Duration: {}ms", ms);
                }
                if let Some(cost) = total_cost_usd {
                    println!("Cost: ${:.4}", cost);
                }
                break;
            }
            Ok(Message::Result(ResultMessage::Error { error, .. })) => {
                println!("\n\nError: {}", error);
                break;
            }
            Err(e) => {
                eprintln!("\nStream error: {}", e);
                break;
            }
            _ => {}
        }
    }

    client.close().await?;
    println!("\nDone — demonstrated interrupt and MCP status.");
    Ok(())
}

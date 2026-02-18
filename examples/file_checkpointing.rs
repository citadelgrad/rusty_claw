//! File checkpointing — enable snapshots and rewind file state.
//!
//! When file checkpointing is enabled, the CLI takes snapshots of modified files
//! at each message boundary. You can then rewind to a previous state using
//! `client.rewind_files(message_id)`.
//!
//! This is useful for:
//! - Undoing changes Claude made to files
//! - Exploring alternative implementations
//! - Safe experimentation with rollback capability
//!
//! ## Run
//! ```sh
//! cargo run -p examples --example file_checkpointing
//! ```

use rusty_claw::prelude::*;
use std::io::{self, Write};
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

    println!("=== File Checkpointing ===\n");

    // Enable file checkpointing in the options
    let options = ClaudeAgentOptions::builder()
        .max_turns(5)
        .model("claude-haiku-4-5")
        .permission_mode(PermissionMode::AcceptEdits)
        .enable_file_checkpointing(true)
        .build();

    println!(
        "File checkpointing enabled: {}",
        options.enable_file_checkpointing
    );

    let mut client = ClaudeClient::new(options)?;
    client.connect().await?;

    // --- Step 1: Ask Claude to create a file ---
    println!("\n--- Step 1: Create a file ---\n");

    let mut stream = client
        .send_message("Create a file /tmp/rusty_claw_checkpoint_test.txt with the text 'original content'. Just create it, no explanation needed.")
        .await?;

    let mut message_count: u32 = 0;
    while let Some(result) = stream.next().await {
        match result {
            Ok(Message::System(SystemMessage::Init { session_id, .. })) => {
                println!("[session: {}]", session_id);
            }
            Ok(Message::Assistant(msg)) => {
                message_count += 1;
                for block in &msg.message.content {
                    if let ContentBlock::Text { text } = block {
                        print!("{}", text);
                        io::stdout().flush()?;
                    }
                }
            }
            Ok(Message::Result(_)) => break,
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
            _ => {}
        }
    }
    println!();

    // --- Step 2: Modify the file ---
    println!("\n--- Step 2: Modify the file ---\n");

    // In a real app, you'd capture the message ID from stream events.
    // The rewind_files() API takes a message ID string.
    let checkpoint_message_id = format!("msg_{}", message_count);
    println!(
        "Checkpoint at message count: {} (ID: {})",
        message_count, checkpoint_message_id
    );

    let mut stream = client
        .send_message("Replace the contents of /tmp/rusty_claw_checkpoint_test.txt with 'modified content'. Just do it, no explanation needed.")
        .await?;

    while let Some(result) = stream.next().await {
        match result {
            Ok(Message::Assistant(msg)) => {
                for block in &msg.message.content {
                    if let ContentBlock::Text { text } = block {
                        print!("{}", text);
                        io::stdout().flush()?;
                    }
                }
            }
            Ok(Message::Result(_)) => break,
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
            _ => {}
        }
    }
    println!();

    // --- Step 3: Rewind ---
    println!("\n--- Step 3: Rewind to checkpoint ---\n");

    if !checkpoint_message_id.is_empty() {
        println!("Rewinding to message: {}", checkpoint_message_id);
        match client.rewind_files(&checkpoint_message_id).await {
            Ok(_) => println!("Rewind successful — files restored to checkpoint state."),
            Err(e) => println!("Rewind failed: {} (this is expected if the CLI doesn't support checkpoints in this mode)", e),
        }
    } else {
        println!("No checkpoint message ID captured — skipping rewind.");
    }

    // Verify the file state
    println!("\n--- Verify ---");
    let contents = std::fs::read_to_string("/tmp/rusty_claw_checkpoint_test.txt")
        .unwrap_or_else(|_| "(file not found)".to_string());
    println!("File contents: {:?}", contents);

    // Clean up
    let _ = std::fs::remove_file("/tmp/rusty_claw_checkpoint_test.txt");
    client.close().await?;

    println!("\nDone — demonstrated file checkpointing and rewind.");
    Ok(())
}

//! Interactive client example demonstrating multi-turn conversations.
//!
//! This example shows how to:
//! - Create and configure a `ClaudeClient` for interactive sessions
//! - Connect and initialize a session
//! - Send multiple messages in a conversation
//! - Use control operations (interrupt, set_model, set_permission_mode)
//! - Stream and process responses
//! - Gracefully close the session
//!
//! # Usage
//!
//! ```bash
//! cargo run --example interactive_client --package rusty_claw
//! ```

use rusty_claw::prelude::*;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Rusty Claw Interactive Client Example ===\n");

    // Step 1: Create client with options
    // ClaudeClient manages a persistent session with Claude
    let options = ClaudeAgentOptions::builder()
        .max_turns(10) // Allow up to 10 turns per message
        .permission_mode(PermissionMode::AcceptEdits) // Auto-accept file edits
        .model("claude-sonnet-4-5".to_string()) // Initial model
        .build();

    println!("Creating client with options:");
    println!("  - Max turns: {:?}", options.max_turns);
    println!("  - Permission mode: {:?}", options.permission_mode);
    println!("  - Model: {:?}", options.model);
    println!();

    let mut client = ClaudeClient::new(options)?;

    // Step 2: Connect and initialize session
    // This establishes the connection to Claude CLI and sends the initialize control request
    println!("Connecting to Claude CLI...");
    client.connect().await?;
    println!("Connected and initialized!\n");

    // Step 3: First message - Ask about the current directory
    println!("=== Turn 1: List files ===");
    let mut stream = client
        .send_message("What files are in this directory?")
        .await?;

    // Process the response stream
    while let Some(result) = stream.next().await {
        match result {
            Ok(Message::Assistant(msg)) => {
                // Print assistant's text responses
                for block in msg.message.content {
                    if let ContentBlock::Text { text } = block {
                        println!("Claude: {}", text);
                    }
                }
            }
            Ok(Message::Result(_)) => {
                println!("(Turn complete)\n");
                break;
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
            _ => {}
        }
    }

    // Step 4: Control operation - Switch model
    println!("=== Switching to Haiku model ===");
    client.set_model("claude-haiku-4-5".to_string()).await?;
    println!("Model changed to claude-haiku-4-5\n");

    // Step 5: Second message - Simple calculation
    println!("=== Turn 2: Calculate ===");
    let mut stream = client.send_message("What is 42 * 137?").await?;

    while let Some(result) = stream.next().await {
        match result {
            Ok(Message::Assistant(msg)) => {
                for block in msg.message.content {
                    if let ContentBlock::Text { text } = block {
                        println!("Claude: {}", text);
                    }
                }
            }
            Ok(Message::Result(_)) => {
                println!("(Turn complete)\n");
                break;
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
            _ => {}
        }
    }

    // Step 6: Control operation - Change permission mode
    println!("=== Changing permission mode to Ask ===");
    client.set_permission_mode(PermissionMode::Ask).await?;
    println!("Permission mode changed to Ask\n");

    // Step 7: Third message - Context from previous conversation
    println!("=== Turn 3: Follow-up question ===");
    let mut stream = client
        .send_message("What was the result of the calculation you just did?")
        .await?;

    while let Some(result) = stream.next().await {
        match result {
            Ok(Message::Assistant(msg)) => {
                for block in msg.message.content {
                    if let ContentBlock::Text { text } = block {
                        println!("Claude: {}", text);
                    }
                }
            }
            Ok(Message::Result(_)) => {
                println!("(Turn complete)\n");
                break;
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
            _ => {}
        }
    }

    // Demonstrate interrupt (commented out to avoid interrupting example)
    // println!("=== Demonstrating Interrupt ===");
    // client.interrupt().await?;
    // println!("Interrupted current execution\n");

    // Demonstrate MCP status query (commented out - requires MCP servers)
    // println!("=== Querying MCP Status ===");
    // let status = client.mcp_status().await?;
    // println!("MCP Status: {:?}\n", status);

    // Demonstrate file rewind (commented out - requires actual file changes)
    // println!("=== Rewinding Files ===");
    // client.rewind_files(Some("message_id".to_string())).await?;
    // println!("Files rewound to previous state\n");

    // Step 8: Close the session gracefully
    println!("=== Closing session ===");
    client.close().await?;
    println!("Session closed gracefully");

    println!("\n=== Example Complete ===");
    println!("This example demonstrated:");
    println!("  - Creating and connecting a ClaudeClient");
    println!("  - Sending multiple messages in a conversation");
    println!("  - Using control operations (set_model, set_permission_mode)");
    println!("  - Maintaining conversation context across turns");
    println!("  - Gracefully closing the session");

    Ok(())
}

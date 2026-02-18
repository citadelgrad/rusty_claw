//! Simple query example demonstrating basic SDK usage.
//!
//! This example shows how to:
//! - Configure options with `ClaudeAgentOptions::builder()`
//! - Execute a one-shot query using `query()`
//! - Stream and handle response messages
//! - Process different message types
//!
//! # Usage
//!
//! ```bash
//! cargo run --example simple_query --package rusty_claw
//! ```

use rusty_claw::prelude::*;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Rusty Claw Simple Query Example ===\n");

    // Configure options using the builder pattern
    // This sets up the agent's behavior, permissions, and model
    let options = ClaudeAgentOptions::builder()
        .max_turns(5) // Limit conversation to 5 turns
        .permission_mode(PermissionMode::AcceptEdits) // Auto-accept file edits
        .model("claude-sonnet-4-5".to_string()) // Specify Claude model
        .build();

    println!("Options configured:");
    println!("  - Max turns: {:?}", options.max_turns);
    println!("  - Permission mode: {:?}", options.permission_mode);
    println!("  - Model: {:?}", options.model);
    println!();

    // Execute a one-shot query
    // The query() function automatically discovers and connects to the Claude CLI
    println!("Sending query: 'What files are in this directory?'\n");
    let mut stream = query("What files are in this directory?", Some(options)).await?;

    // Stream responses from Claude
    // The stream yields Result<Message, ClawError> items
    while let Some(result) = stream.next().await {
        match result {
            // Assistant message contains Claude's response
            Ok(Message::Assistant(msg)) => {
                println!("=== Assistant Message ===");
                // Process each content block in the message
                for block in msg.message.content {
                    match block {
                        ContentBlock::Text { text } => {
                            println!("Claude: {}", text);
                        }
                        ContentBlock::ToolUse { id, name, input } => {
                            println!("Tool use: {} (ID: {})", name, id);
                            println!("Input: {}", input);
                        }
                        ContentBlock::ToolResult { tool_use_id, content, is_error } => {
                            println!("Tool result (ID: {}, error: {}): {:?}", tool_use_id, is_error, content);
                        }
                        ContentBlock::Thinking { thinking } => {
                            println!("Thinking: {}", thinking);
                        }
                    }
                }
                println!();
            }
            // Result message indicates query completion
            Ok(Message::Result(msg)) => {
                println!("=== Query Complete ===");
                println!("Result: {:?}", msg);
                break;
            }
            // System messages (metadata, status updates)
            Ok(Message::System(msg)) => {
                println!("System: {:?}", msg);
            }
            // User messages (echo of what was sent)
            Ok(Message::User(msg)) => {
                println!("User: {:?}", msg);
            }
            // Control messages (internal protocol - not normally seen in query())
            Ok(Message::ControlRequest { .. }) | Ok(Message::ControlResponse { .. })
            | Ok(Message::RateLimitEvent(_)) | Ok(Message::McpMessage(_)) => {
                // Internal protocol messages - not normally seen in query()
            }
            // Handle errors
            Err(e) => {
                eprintln!("Error: {}", e);
                break;
            }
        }
    }

    println!("\n=== Example Complete ===");
    Ok(())
}

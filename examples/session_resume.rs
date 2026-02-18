//! Session resume — reconnect to, fork, and name sessions.
//!
//! This example demonstrates the session management APIs:
//! - `.resume()` — reconnect to an existing session by ID
//! - `.fork_session()` — branch off a session to explore alternatives
//! - `.session_name()` — assign a human-readable name for management
//!
//! ## Run
//! ```sh
//! cargo run -p examples --example session_resume
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

    // --- 1. Named session ---
    // Assign a name so you can find this session later in Claude's session list.
    println!("=== Named Session ===");
    let options = ClaudeAgentOptions::builder()
        .max_turns(3)
        .model("claude-haiku-4-5")
        .permission_mode(PermissionMode::AcceptEdits)
        .session_name("demo-session")
        .build();

    println!("Session name: {:?}", options.session_name);

    let mut client = ClaudeClient::new(options)?;
    client.connect().await?;

    let mut stream = client
        .send_message("Say hello and tell me my session name.")
        .await?;
    let mut session_id = String::new();

    while let Some(result) = stream.next().await {
        match result {
            Ok(Message::System(SystemMessage::Init {
                session_id: sid, ..
            })) => {
                println!("[session: {}]", sid);
                session_id = sid;
            }
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
    client.close().await?;

    if session_id.is_empty() {
        println!("\nNo session ID received — cannot demonstrate resume/fork.");
        println!("Run with RUST_LOG=debug to diagnose.");
        return Ok(());
    }

    // --- 2. Resume session ---
    // Reconnect to the same session to continue the conversation.
    println!("\n=== Resume Session ===");
    println!("Resuming session: {}", session_id);

    let resume_options = ClaudeAgentOptions::builder()
        .max_turns(3)
        .model("claude-haiku-4-5")
        .permission_mode(PermissionMode::AcceptEdits)
        .resume(&session_id)
        .build();

    let mut client = ClaudeClient::new(resume_options)?;
    client.connect().await?;

    let mut stream = client
        .send_message("What was the first thing I said?")
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
    client.close().await?;

    // --- 3. Fork session ---
    // Branch the session so the original stays untouched.
    println!("\n=== Fork Session ===");
    println!("Forking session: {}", session_id);

    let fork_options = ClaudeAgentOptions::builder()
        .max_turns(3)
        .model("claude-haiku-4-5")
        .permission_mode(PermissionMode::AcceptEdits)
        .resume(&session_id)
        .fork_session(true)
        .build();

    let mut client = ClaudeClient::new(fork_options)?;
    client.connect().await?;

    let mut stream = client
        .send_message("This is a forked conversation. What was the original topic?")
        .await?;

    while let Some(result) = stream.next().await {
        match result {
            Ok(Message::System(SystemMessage::Init {
                session_id: sid, ..
            })) => {
                println!("[forked session: {}]", sid);
            }
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
    client.close().await?;

    println!("\nDone — demonstrated named session, resume, and fork.");
    Ok(())
}

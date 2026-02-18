//! Agent environment — configure working directory, env vars, and CLI path.
//!
//! This example demonstrates environment configuration options:
//! - `.cwd()` — set the working directory for the CLI subprocess
//! - `.env()` — pass custom environment variables
//! - `.cli_path()` — specify an explicit path to the Claude CLI binary
//!
//! Note: `SandboxSettings` is a unit struct placeholder and is not demonstrated here.
//!
//! ## Run
//! ```sh
//! cargo run -p examples --example agent_environment
//! ```

use rusty_claw::prelude::*;
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::PathBuf;
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

    println!("=== Agent Environment Configuration ===\n");

    // --- 1. Custom working directory ---
    println!("--- Working Directory ---");

    let cwd = std::env::current_dir()?;
    println!("Current dir: {}", cwd.display());

    let options_with_cwd = ClaudeAgentOptions::builder()
        .max_turns(3)
        .model("claude-haiku-4-5")
        .permission_mode(PermissionMode::AcceptEdits)
        .cwd(PathBuf::from("/tmp"))
        .build();

    println!("Configured cwd: {:?}\n", options_with_cwd.cwd);

    // --- 2. Custom environment variables ---
    println!("--- Environment Variables ---");

    let mut env = HashMap::new();
    env.insert("PROJECT_NAME".to_string(), "rusty-claw-demo".to_string());
    env.insert("BUILD_MODE".to_string(), "development".to_string());
    env.insert("LOG_LEVEL".to_string(), "debug".to_string());

    let options_with_env = ClaudeAgentOptions::builder()
        .max_turns(3)
        .model("claude-haiku-4-5")
        .permission_mode(PermissionMode::AcceptEdits)
        .env(env)
        .build();

    println!("Configured env vars:");
    for (k, v) in &options_with_env.env {
        println!("  {}={}", k, v);
    }
    println!();

    // --- 3. Explicit CLI path ---
    println!("--- CLI Path Discovery ---");

    // Auto-discover the CLI
    let discovered = CliDiscovery::find(None).await;
    match &discovered {
        Ok(path) => println!("Auto-discovered CLI: {}", path.display()),
        Err(e) => println!("CLI not found: {}", e),
    }

    // Explicitly set the CLI path (useful in CI/containers)
    let explicit_path = discovered.unwrap_or_else(|_| PathBuf::from("/usr/local/bin/claude"));

    let options_with_cli = ClaudeAgentOptions::builder()
        .max_turns(3)
        .model("claude-haiku-4-5")
        .permission_mode(PermissionMode::AcceptEdits)
        .cli_path(&explicit_path)
        .build();

    println!("Configured CLI path: {:?}\n", options_with_cli.cli_path);

    // --- 4. All together ---
    println!("--- Combined Configuration ---");

    let mut env = HashMap::new();
    env.insert("MY_APP_CONFIG".to_string(), "production".to_string());

    let combined = ClaudeAgentOptions::builder()
        .max_turns(3)
        .model("claude-haiku-4-5")
        .permission_mode(PermissionMode::AcceptEdits)
        .cwd(std::env::current_dir()?)
        .env(env)
        .cli_path(&explicit_path)
        .build();

    // Show the CLI args generated
    let args = combined.to_cli_args("test prompt");
    println!("Generated CLI args:");
    for arg in &args {
        println!("  {}", arg);
    }
    println!();

    // --- 5. Live demo ---
    println!("--- Live Demo ---\n");

    let mut client = ClaudeClient::new(combined)?;
    client.connect().await?;

    let mut stream = client
        .send_message("What is the current working directory? Answer in one sentence.")
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
    println!("\nDone — demonstrated cwd, env, and cli_path configuration.");
    Ok(())
}

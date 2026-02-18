//! Transport layer — low-level CLI discovery and transport setup.
//!
//! This example demonstrates the transport layer API, which is normally
//! hidden behind `ClaudeClient`. Use these APIs when you need:
//! - Manual CLI discovery and version validation
//! - Custom transport configurations
//! - Direct control over the subprocess lifecycle
//!
//! APIs demonstrated:
//! - `CliDiscovery::find()` — locate the Claude CLI binary
//! - `CliDiscovery::validate_version()` — ensure CLI version >= 2.0.0
//! - `SubprocessCLITransport::new()` — create a transport
//! - `Transport` trait — connect, write, messages, close
//!
//! ## Run
//! ```sh
//! cargo run -p examples --example transport_layer
//! ```

use rusty_claw::prelude::*;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rusty_claw=warn".parse().unwrap()),
        )
        .with_target(false)
        .init();

    println!("=== Transport Layer ===\n");

    // --- 1. CLI Discovery ---
    println!("--- CLI Discovery ---\n");

    // Auto-discover from PATH / common locations
    match CliDiscovery::find(None).await {
        Ok(path) => {
            println!("Found CLI: {}", path.display());

            // Validate version
            match CliDiscovery::validate_version(&path).await {
                Ok(version) => println!("Version: {} (OK, >= 2.0.0)", version),
                Err(e) => println!("Version check failed: {}", e),
            }
        }
        Err(ClawError::CliNotFound) => {
            println!("CLI not found in PATH or common locations.");
            println!("Install: npm install -g @anthropic-ai/claude-code");
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    }

    // Try explicit path
    let explicit = PathBuf::from("/opt/homebrew/bin/claude");
    match CliDiscovery::find(Some(&explicit)).await {
        Ok(path) => println!("Explicit path found: {}", path.display()),
        Err(_) => println!("Explicit path {} not found", explicit.display()),
    }

    // Check CLAUDE_CLI_PATH env var
    if let Ok(env_path) = std::env::var("CLAUDE_CLI_PATH") {
        println!("CLAUDE_CLI_PATH: {}", env_path);
    } else {
        println!("CLAUDE_CLI_PATH: (not set)");
    }

    // --- 2. Transport Creation ---
    println!("\n--- SubprocessCLITransport ---\n");

    // The transport is what ClaudeClient uses internally.
    // You can create it manually for low-level control.
    let transport = SubprocessCLITransport::new(
        None, // Auto-discover CLI
        vec![
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--verbose".to_string(),
        ],
    );

    println!("Transport created (not yet connected).");
    println!("is_ready: {}", transport.is_ready());

    // --- 3. Transport Trait ---
    println!("\n--- Transport Trait Methods ---\n");
    println!("The Transport trait defines the subprocess lifecycle:");
    println!("  1. transport.connect()    — spawn CLI, establish pipes");
    println!("  2. transport.write(bytes) — send JSON messages to stdin");
    println!("  3. transport.messages()   — get mpsc::UnboundedReceiver for stdout");
    println!("  4. transport.end_input()  — close stdin (signal no more input)");
    println!("  5. transport.close()      — graceful shutdown (SIGTERM → SIGKILL)");
    println!("  6. transport.is_ready()   — check if connected and alive");
    println!();
    println!("Custom transports can implement this trait for:");
    println!("  - Mock transports for testing");
    println!("  - Remote connections (SSH, WebSocket)");
    println!("  - Alternative CLI implementations");

    // --- 4. Comparison with ClaudeClient ---
    println!("\n--- ClaudeClient vs Manual Transport ---\n");
    println!("ClaudeClient wraps the transport and adds:");
    println!("  - Control protocol handling (initialize, permissions)");
    println!("  - MCP message routing to registered handlers");
    println!("  - Message parsing (raw JSON → typed Message enum)");
    println!("  - Session management (resume, fork, named sessions)");
    println!();
    println!("Use ClaudeClient for normal usage. Use transport directly only when");
    println!("you need custom protocol handling or testing.\n");

    println!("Done — demonstrated CLI discovery and transport layer.");
    Ok(())
}

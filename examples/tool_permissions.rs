//! Tool permissions — control which tools the agent can use.
//!
//! This example demonstrates the permission system:
//! - `.allowed_tools()` / `.disallowed_tools()` — static allow/deny lists
//! - `CanUseToolHandler` trait — programmatic runtime permission checks
//! - `DefaultPermissionHandler` — pre-built handler with builder pattern
//!
//! ## Run
//! ```sh
//! cargo run -p examples --example tool_permissions
//! ```

use async_trait::async_trait;
use rusty_claw::prelude::*;
use serde_json::json;
use std::sync::Arc;

/// Custom permission handler that blocks write operations and allows reads.
struct ReadOnlyPermissionHandler;

#[async_trait]
impl CanUseToolHandler for ReadOnlyPermissionHandler {
    async fn can_use_tool(
        &self,
        tool_name: &str,
        tool_input: &serde_json::Value,
    ) -> Result<bool, ClawError> {
        // Allow read-only tools
        let read_tools = ["Read", "Glob", "Grep", "LSP"];
        if read_tools.contains(&tool_name) {
            println!("  [Permission] ALLOW: {} (read-only tool)", tool_name);
            return Ok(true);
        }

        // Allow Bash only for safe commands
        if tool_name == "Bash" {
            if let Some(cmd) = tool_input.get("command").and_then(|v| v.as_str()) {
                let safe = !cmd.contains("rm ") && !cmd.contains("mv ") && !cmd.contains("> ");
                if safe {
                    println!("  [Permission] ALLOW: Bash (safe command: {})", cmd);
                    return Ok(true);
                }
                println!("  [Permission] DENY:  Bash (unsafe command: {})", cmd);
                return Ok(false);
            }
        }

        // Deny everything else
        println!("  [Permission] DENY:  {} (not in allowlist)", tool_name);
        Ok(false)
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

    // --- 1. Static allow/deny lists ---
    println!("=== Static Permission Lists ===\n");

    let options = ClaudeAgentOptions::builder()
        .allowed_tools(vec![
            "Read".to_string(),
            "Glob".to_string(),
            "Grep".to_string(),
        ])
        .disallowed_tools(vec!["Write".to_string(), "Edit".to_string()])
        .permission_mode(PermissionMode::AcceptEdits)
        .build();

    println!("Allowed tools:    {:?}", options.allowed_tools);
    println!("Disallowed tools: {:?}", options.disallowed_tools);

    // The CLI enforces these lists — only allowed tools are available,
    // and disallowed tools are explicitly blocked.
    let args = options.to_cli_args("test");
    println!("CLI args include:  --allowed-tools and --disallowed-tools\n");
    let _ = args; // used for demonstration

    // --- 2. DefaultPermissionHandler ---
    println!("=== DefaultPermissionHandler ===\n");

    // Pre-built handler with deny-by-default + allowlist
    let default_handler = DefaultPermissionHandler::builder()
        .mode(PermissionMode::Deny)
        .allowed_tools(vec!["Read".to_string(), "Grep".to_string()])
        .disallowed_tools(vec!["Bash".to_string()])
        .build();

    // Test the handler
    let test_cases = vec![
        ("Read", json!({}), true),
        ("Grep", json!({}), true),
        ("Bash", json!({}), false),  // Explicitly denied
        ("Write", json!({}), false), // Not in allowlist + deny mode
    ];

    for (tool, input, expected) in &test_cases {
        let result = default_handler.can_use_tool(tool, input).await?;
        println!(
            "  DefaultPermissionHandler.can_use_tool({:?}) = {} (expected: {})",
            tool, result, expected
        );
        assert_eq!(result, *expected);
    }

    // --- 3. Programmatic CanUseToolHandler ---
    println!("\n=== Custom CanUseToolHandler ===\n");

    let handler = ReadOnlyPermissionHandler;

    let test_cases = vec![
        ("Read", json!({})),
        ("Glob", json!({})),
        ("Bash", json!({"command": "ls -la"})),
        ("Bash", json!({"command": "rm -rf /tmp/test"})),
        ("Write", json!({"file_path": "/tmp/test.txt"})),
    ];

    for (tool, input) in &test_cases {
        let result = handler.can_use_tool(tool, input).await?;
        println!("    → {}\n", if result { "ALLOWED" } else { "DENIED" });
    }

    // --- 4. Register with client ---
    println!("=== Register Handler with Client ===\n");

    let options = ClaudeAgentOptions::builder()
        .max_turns(3)
        .permission_mode(PermissionMode::AcceptEdits)
        .build();

    let client = ClaudeClient::new(options)?;

    // Register the custom handler (takes effect after connect())
    let handler = Arc::new(ReadOnlyPermissionHandler);
    client.register_can_use_tool_handler(handler).await;

    println!("Handler registered. After connect(), the CLI will consult");
    println!("the handler for every tool use request.\n");

    println!("Done — demonstrated static lists, DefaultPermissionHandler, and custom handler.");
    Ok(())
}

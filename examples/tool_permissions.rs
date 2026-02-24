//! Tool permissions — control which tools the agent can use.
//!
//! This example demonstrates the permission system:
//! - `.allowed_tools()` / `.disallowed_tools()` — static allow/deny lists
//! - `CanUseToolHandler` trait — programmatic runtime permission checks
//! - `DefaultPermissionHandler` — pre-built handler with builder pattern
//! - `PermissionDecision` — rich result type with input mutation support
//!
//! ## Run
//! ```sh
//! cargo run -p examples --example tool_permissions
//! ```

use async_trait::async_trait;
use rusty_claw::permissions::PermissionDecision;
use rusty_claw::prelude::*;
use serde_json::json;
use std::sync::Arc;

/// Custom permission handler that blocks write operations and allows reads.
#[derive(Debug)]
struct ReadOnlyPermissionHandler;

#[async_trait]
impl CanUseToolHandler for ReadOnlyPermissionHandler {
    async fn can_use_tool(
        &self,
        tool_name: &str,
        tool_input: &serde_json::Value,
    ) -> Result<PermissionDecision, ClawError> {
        // Allow read-only tools
        let read_tools = ["Read", "Glob", "Grep", "LSP"];
        if read_tools.contains(&tool_name) {
            println!("  [Permission] ALLOW: {} (read-only tool)", tool_name);
            return Ok(PermissionDecision::Allow { updated_input: None });
        }

        // Allow Bash only for safe commands
        if tool_name == "Bash"
            && let Some(cmd) = tool_input.get("command").and_then(|v| v.as_str())
        {
            let safe = !cmd.contains("rm ") && !cmd.contains("mv ") && !cmd.contains("> ");
            if safe {
                println!("  [Permission] ALLOW: Bash (safe command: {})", cmd);
                return Ok(PermissionDecision::Allow { updated_input: None });
            }
            println!("  [Permission] DENY:  Bash (unsafe command: {})", cmd);
            return Ok(PermissionDecision::Deny { interrupt: false });
        }

        // Deny everything else
        println!("  [Permission] DENY:  {} (not in allowlist)", tool_name);
        Ok(PermissionDecision::Deny { interrupt: false })
    }
}

/// Handler that sanitizes Bash commands by stripping dangerous flags.
#[derive(Debug)]
struct SanitizingHandler;

#[async_trait]
impl CanUseToolHandler for SanitizingHandler {
    async fn can_use_tool(
        &self,
        tool_name: &str,
        tool_input: &serde_json::Value,
    ) -> Result<PermissionDecision, ClawError> {
        if tool_name == "Bash"
            && let Some(cmd) = tool_input.get("command").and_then(|v| v.as_str())
            && cmd.contains("dangerous")
        {
            // Replace unsafe command with a safe echo instead
            let sanitized = json!({ "command": "echo 'command sanitized'" });
            println!("  [Sanitize] Replacing dangerous command with safe echo");
            return Ok(PermissionDecision::Allow {
                updated_input: Some(sanitized),
            });
        }
        Ok(PermissionDecision::Allow { updated_input: None })
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

    let args = options.to_cli_args("test");
    println!("CLI args include:  --allowed-tools and --disallowed-tools\n");
    let _ = args;

    // --- 2. DefaultPermissionHandler ---
    println!("=== DefaultPermissionHandler ===\n");

    let default_handler = DefaultPermissionHandler::builder()
        .mode(PermissionMode::Deny)
        .allowed_tools(vec!["Read".to_string(), "Grep".to_string()])
        .disallowed_tools(vec!["Bash".to_string()])
        .build();

    let test_cases: Vec<(&str, serde_json::Value, bool)> = vec![
        ("Read", json!({}), true),
        ("Grep", json!({}), true),
        ("Bash", json!({}), false),
        ("Write", json!({}), false),
    ];

    for (tool, input, expected_allow) in &test_cases {
        let result = default_handler.can_use_tool(tool, input).await?;
        let allowed = result.is_allowed();
        println!(
            "  DefaultPermissionHandler.can_use_tool({:?}) = {} (expected: {})",
            tool,
            if allowed { "ALLOW" } else { "DENY" },
            if *expected_allow { "ALLOW" } else { "DENY" },
        );
        assert_eq!(allowed, *expected_allow);
    }

    // --- 3. Programmatic CanUseToolHandler ---
    println!("\n=== Custom CanUseToolHandler ===\n");

    let handler = ReadOnlyPermissionHandler;

    let test_cases = vec![
        ("Read", json!({})),
        ("Glob", json!({})),
        ("Bash", json!({"command": "ls -la"})),
        ("Bash", json!({"command": "rm /tmp/test"})),
        ("Write", json!({"file_path": "/tmp/test.txt"})),
    ];

    for (tool, input) in &test_cases {
        let result = handler.can_use_tool(tool, input).await?;
        println!(
            "    {} {}\n",
            tool,
            if result.is_allowed() { "ALLOWED" } else { "DENIED" }
        );
    }

    // --- 4. Input mutation (PermissionDecision::Allow { updated_input }) ---
    println!("=== Input Mutation with PermissionDecision ===\n");

    let sanitizer = SanitizingHandler;
    let result = sanitizer
        .can_use_tool("Bash", &json!({"command": "run dangerous script"}))
        .await?;
    match &result {
        PermissionDecision::Allow { updated_input: Some(input) } => {
            println!("  Sanitized input: {}", input);
        }
        PermissionDecision::Allow { updated_input: None } => {
            println!("  Allowed with original input");
        }
        PermissionDecision::Deny { interrupt } => {
            println!("  Denied (interrupt={})", interrupt);
        }
    }

    // --- 5. Register with client via options ---
    println!("\n=== Register Handler via ClaudeAgentOptions ===\n");

    let handler = ReadOnlyPermissionHandler;
    let options = ClaudeAgentOptions::builder()
        .max_turns(3)
        .permission_mode(PermissionMode::AcceptEdits)
        .permission_handler(handler)
        .build();

    println!("Handler set in options. It will be registered before connect().");
    let _client = ClaudeClient::new(options)?;

    // --- 6. Register with client directly ---
    println!("\n=== Register Handler Directly with Client ===\n");

    let options2 = ClaudeAgentOptions::builder()
        .max_turns(3)
        .permission_mode(PermissionMode::AcceptEdits)
        .build();

    let client = ClaudeClient::new(options2)?;
    let handler = Arc::new(ReadOnlyPermissionHandler);
    client.register_can_use_tool_handler(handler).await;

    println!("Handler registered. After connect(), the CLI will consult");
    println!("the handler for every tool use request.\n");

    println!("Done — demonstrated static lists, DefaultPermissionHandler, custom handler, and input mutation.");
    Ok(())
}

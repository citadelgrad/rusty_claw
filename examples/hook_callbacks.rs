//! Hook callbacks — implement event-driven permission and context hooks.
//!
//! This example demonstrates the hook system:
//! - `HookCallback` trait — implement as a struct or async function
//! - `HookInput` — tool name, input, output, error, prompt data
//! - `HookContext` — session ID, available tools, agents, MCP servers
//! - `HookResponse` — allow/deny/ask with context injection
//! - `HookMatcher` — pattern matching for selective hook triggering
//!
//! ## Run
//! ```sh
//! cargo run -p examples --example hook_callbacks
//! ```

use async_trait::async_trait;
use rusty_claw::prelude::*;
use serde_json::json;

// --- 1. Struct-based HookCallback ---
// Best for stateful hooks that need configuration.

/// A security hook that blocks dangerous Bash commands.
struct SecurityHook {
    blocked_patterns: Vec<String>,
}

#[async_trait]
impl HookCallback for SecurityHook {
    async fn call(
        &self,
        input: HookInput,
        _tool_use_id: Option<&str>,
        _context: &HookContext,
    ) -> Result<HookResponse, ClawError> {
        if let Some(tool_name) = &input.tool_name {
            if tool_name == "Bash" {
                if let Some(tool_input) = &input.tool_input {
                    if let Some(cmd) = tool_input.get("command").and_then(|v| v.as_str()) {
                        for pattern in &self.blocked_patterns {
                            if cmd.contains(pattern.as_str()) {
                                return Ok(HookResponse::deny(format!(
                                    "Blocked: command contains '{}'",
                                    pattern
                                )));
                            }
                        }
                    }
                }
            }
        }
        Ok(HookResponse::allow("Passed security check"))
    }
}

// --- 2. Function-based HookCallback ---
// Best for simple, stateless hooks. The blanket impl on Fn makes this work.

async fn logging_hook(
    input: HookInput,
    tool_use_id: Option<&str>,
    context: &HookContext,
) -> Result<HookResponse, ClawError> {
    println!("  [Hook] Tool: {:?}", input.tool_name);
    println!("  [Hook] Tool Use ID: {:?}", tool_use_id);
    println!("  [Hook] Session: {:?}", context.session_id);

    // Always allow, but inject context
    Ok(HookResponse::allow("Logged")
        .with_context("This operation was logged by the SDK hook system."))
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

    println!("=== Hook Callbacks ===\n");

    // --- HookInput constructors ---
    println!("--- HookInput Constructors ---\n");

    let tool_use_input = HookInput::tool_use("Bash", json!({"command": "ls -la"}));
    println!("tool_use:    {:?}", tool_use_input.tool_name);

    let tool_success_input = HookInput::tool_success("Bash", json!({"output": "file.txt"}));
    println!("tool_success: {:?}", tool_success_input.tool_output);

    let tool_failure_input = HookInput::tool_failure("Bash", "Permission denied");
    println!("tool_failure: {:?}", tool_failure_input.error);

    let prompt_input = HookInput::prompt("What files are in this directory?");
    println!("prompt:      {:?}", prompt_input.prompt);

    // --- HookContext builder ---
    println!("\n--- HookContext Builder ---\n");

    let context = HookContext::with_session("session-abc123")
        .with_tools(vec![
            "Bash".to_string(),
            "Read".to_string(),
            "Write".to_string(),
        ])
        .with_agents(vec!["researcher".to_string()])
        .with_mcp_servers(vec!["text_tools".to_string()]);

    println!("Session:     {:?}", context.session_id);
    println!("Tools:       {:?}", context.available_tools);
    println!("Agents:      {:?}", context.agents);
    println!("MCP servers: {:?}", context.mcp_servers);

    // --- HookResponse patterns ---
    println!("\n--- HookResponse Patterns ---\n");

    let allow = HookResponse::allow("Safe operation");
    println!(
        "allow: decision={:?} reason={:?}",
        allow.permission_decision, allow.permission_decision_reason
    );

    let deny = HookResponse::deny("Dangerous operation");
    println!(
        "deny:  decision={:?} continue={}",
        deny.permission_decision, deny.should_continue
    );

    let ask = HookResponse::ask("Confirm destructive operation?");
    println!(
        "ask:   decision={:?} reason={:?}",
        ask.permission_decision, ask.permission_decision_reason
    );

    // Builder pattern for complex responses
    let complex = HookResponse::default()
        .with_permission(PermissionDecision::Allow)
        .with_reason("Approved after review")
        .with_context("Additional context for Claude: user confirmed")
        .with_continue(true)
        .with_updated_input(json!({"command": "ls -la --color=never"}));
    println!(
        "complex: context={:?} updated_input={:?}",
        complex.additional_context, complex.updated_input
    );

    // --- HookMatcher patterns ---
    println!("\n--- HookMatcher Patterns ---\n");

    let all_matcher = HookMatcher::all();
    println!(
        "all() matches 'Bash': {}, 'Read': {}",
        all_matcher.matches("Bash"),
        all_matcher.matches("Read")
    );

    let bash_matcher = HookMatcher::tool("Bash");
    println!(
        "tool('Bash') matches 'Bash': {}, 'Read': {}",
        bash_matcher.matches("Bash"),
        bash_matcher.matches("Read")
    );

    // --- Test struct-based callback ---
    println!("\n--- SecurityHook (struct-based) ---\n");

    let security = SecurityHook {
        blocked_patterns: vec![
            "rm -rf".to_string(),
            "sudo".to_string(),
            "> /dev/".to_string(),
        ],
    };

    let test_commands = vec![
        ("ls -la", true),
        ("rm -rf /", false),
        ("sudo apt install", false),
        ("echo hello", true),
    ];

    for (cmd, expected_allow) in test_commands {
        let input = HookInput::tool_use("Bash", json!({"command": cmd}));
        let response = security.call(input, None, &context).await?;
        let allowed = matches!(
            response.permission_decision,
            Some(PermissionDecision::Allow)
        );
        println!(
            "  {:30} → {} (expected: {})",
            cmd,
            if allowed { "ALLOW" } else { "DENY" },
            if expected_allow { "ALLOW" } else { "DENY" }
        );
        assert_eq!(allowed, expected_allow);
    }

    // --- Test function-based callback ---
    println!("\n--- logging_hook (function-based) ---\n");

    let input = HookInput::tool_use("Read", json!({"file_path": "/tmp/test.txt"}));
    let response = logging_hook(input, Some("toolu_123"), &context).await?;
    println!("  Result: {:?}\n", response.permission_decision);

    // --- HookEvent configuration ---
    println!("--- HookEvent + HookMatcher in Options ---\n");

    let mut hooks = std::collections::HashMap::new();
    hooks.insert(
        HookEvent::PreToolUse,
        vec![HookMatcher::tool("Bash"), HookMatcher::tool("Write")],
    );
    hooks.insert(HookEvent::PostToolUse, vec![HookMatcher::all()]);

    let options = ClaudeAgentOptions::builder()
        .hooks(hooks)
        .permission_mode(PermissionMode::AcceptEdits)
        .build();

    println!("Configured hooks:");
    for (event, matchers) in &options.hooks {
        let tools: Vec<_> = matchers
            .iter()
            .map(|m| m.tool_name.as_deref().unwrap_or("*"))
            .collect();
        println!("  {:?} → {:?}", event, tools);
    }

    println!("\nDone — demonstrated HookCallback, HookInput, HookContext, and HookResponse.");
    Ok(())
}

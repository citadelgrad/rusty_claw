//! Hooks and guardrails example demonstrating the hook system.
//!
//! This example shows how to:
//! - Implement custom hooks using the `HookHandler` trait
//! - Configure hook matchers for specific tools or all tools
//! - Register hooks with `ClaudeClient`
//! - Use hooks for validation (guardrails)
//! - Use hooks for logging and monitoring
//! - Handle different hook events
//!
//! # Usage
//!
//! ```bash
//! cargo run -p examples --example hooks_guardrails
//! ```
//!
//! Note: This example demonstrates hook setup but doesn't actually run queries.
//! To use these hooks, they would need to be registered with a ClaudeClient
//! and then used during actual Claude interactions.

use async_trait::async_trait;
use rusty_claw::prelude::*;
use serde_json::{json, Value};
use std::sync::Arc;

// Hook 1: Guardrail hook - Validates tool usage to block dangerous operations
// This hook checks tool inputs before execution and can block unsafe commands
struct GuardrailHook;

#[async_trait]
impl HookHandler for GuardrailHook {
    async fn call(&self, _event: HookEvent, input: Value) -> Result<Value, ClawError> {
        println!("  [Guardrail] Validating tool use...");

        // Extract tool information from hook input
        let tool_name = input["tool_name"].as_str().unwrap_or("unknown");
        let tool_input = &input["tool_input"];

        println!("    Tool: {}", tool_name);

        // Apply validation rules based on tool type
        match tool_name {
            "Bash" => {
                // Check for dangerous bash commands
                if let Some(command) = tool_input["command"].as_str() {
                    println!("    Command: {}", command);

                    // Block destructive commands
                    let dangerous_patterns = vec![
                        "rm -rf /",
                        "rm -rf /*",
                        "> /dev/sda",
                        "dd if=/dev/zero",
                        "mkfs.",
                        ":(){ :|:& };:", // Fork bomb
                    ];

                    for pattern in dangerous_patterns {
                        if command.contains(pattern) {
                            println!("    ❌ BLOCKED: Dangerous command pattern detected");
                            return Ok(json!({
                                "approved": false,
                                "reason": format!("Dangerous command pattern: {}", pattern)
                            }));
                        }
                    }

                    println!("    ✅ APPROVED");
                    Ok(json!({"approved": true}))
                } else {
                    println!("    ✅ APPROVED (no command)");
                    Ok(json!({"approved": true}))
                }
            }
            "Write" => {
                // Validate file write operations
                if let Some(path) = tool_input["file_path"].as_str() {
                    println!("    Path: {}", path);

                    // Block writes to sensitive system files
                    let blocked_paths =
                        vec!["/etc/passwd", "/etc/shadow", "/boot/", "/sys/", "/proc/"];

                    for blocked in blocked_paths {
                        if path.starts_with(blocked) {
                            println!("    ❌ BLOCKED: Protected path");
                            return Ok(json!({
                                "approved": false,
                                "reason": format!("Cannot write to protected path: {}", blocked)
                            }));
                        }
                    }

                    println!("    ✅ APPROVED");
                    Ok(json!({"approved": true}))
                } else {
                    println!("    ✅ APPROVED (no path)");
                    Ok(json!({"approved": true}))
                }
            }
            _ => {
                // Allow all other tools by default
                println!("    ✅ APPROVED (unrestricted tool)");
                Ok(json!({"approved": true}))
            }
        }
    }
}

// Hook 2: Logging hook - Tracks all tool usage for monitoring and debugging
// This hook logs tool calls without blocking execution
struct LoggingHook;

#[async_trait]
impl HookHandler for LoggingHook {
    async fn call(&self, event: HookEvent, input: Value) -> Result<Value, ClawError> {
        println!("  [Logger] Event: {:?}", event);

        match event {
            HookEvent::PreToolUse => {
                let tool_name = input["tool_name"].as_str().unwrap_or("unknown");
                let tool_input = &input["tool_input"];

                println!("    Tool: {}", tool_name);
                println!("    Input: {}", tool_input);
                // In a real application, you might:
                // - Write to a log file
                // - Send to a monitoring system
                // - Update metrics/counters
                // - Store in a database
            }
            HookEvent::UserPromptSubmit => {
                println!("    User prompt submitted");
            }
            HookEvent::Stop => {
                println!("    Session stopped");
            }
            HookEvent::SubagentStart => {
                println!("    Subagent started: {:?}", input);
            }
            HookEvent::SubagentStop => {
                println!("    Subagent stopped: {:?}", input);
            }
            _ => {
                println!("    Event data: {:?}", input);
            }
        }

        // Logging hooks always approve (they don't block execution)
        Ok(json!({"approved": true}))
    }
}

// Hook 3: Rate limit hook - Enforces rate limits on tool calls
// This hook tracks usage and can block excessive tool calls
struct RateLimitHook {
    max_calls_per_minute: u32,
    // In a real implementation, you'd track:
    // - call_counts: Arc<Mutex<HashMap<String, Vec<Instant>>>>
}

#[async_trait]
impl HookHandler for RateLimitHook {
    async fn call(&self, _event: HookEvent, input: Value) -> Result<Value, ClawError> {
        println!("  [RateLimit] Checking rate limit...");

        let tool_name = input["tool_name"].as_str().unwrap_or("unknown");
        println!("    Tool: {}", tool_name);
        println!("    Limit: {} calls/minute", self.max_calls_per_minute);

        // In a real implementation, you would:
        // 1. Get current timestamp
        // 2. Clean up old timestamps (older than 1 minute)
        // 3. Count recent calls for this tool
        // 4. If count >= max_calls_per_minute, block the call
        // 5. Otherwise, record this call and approve

        // For this example, we'll just approve everything
        println!("    ✅ APPROVED (within rate limit)");
        Ok(json!({"approved": true}))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Rusty Claw Hooks & Guardrails Example ===\n");

    // Step 1: Configure hook matchers
    // Hook matchers specify which tools/events trigger which hooks
    println!("=== Configuring Hook Matchers ===");

    let mut hooks = std::collections::HashMap::new();

    // Match Bash tool use - apply guardrail validation
    println!("PreToolUse event:");
    println!("  - Bash tool → Guardrail validation");
    hooks.insert(HookEvent::PreToolUse, vec![HookMatcher::tool("Bash")]);

    // Match all for UserPromptSubmit event - logging
    println!("UserPromptSubmit event:");
    println!("  - All → Logging");
    hooks.insert(HookEvent::UserPromptSubmit, vec![HookMatcher::all()]);

    // Match all for Stop event - logging
    println!("Stop event:");
    println!("  - All → Logging");
    hooks.insert(HookEvent::Stop, vec![HookMatcher::all()]);

    println!();

    // Step 2: Create options with hook matchers
    let options = ClaudeAgentOptions::builder()
        .max_turns(10)
        .permission_mode(PermissionMode::AcceptEdits)
        .hooks(hooks)
        .build();

    println!("=== Creating Client with Hooks ===");
    let client = ClaudeClient::new(options)?;

    // Step 3: Register hook handlers
    // Each hook ID maps to a specific hook implementation
    println!("Registering hook handlers:");

    let guardrail = Arc::new(GuardrailHook);
    println!("  - guardrail → GuardrailHook (validates tool inputs)");
    client
        .register_hook("guardrail".to_string(), guardrail)
        .await;

    let logger = Arc::new(LoggingHook);
    println!("  - logger → LoggingHook (logs all tool usage)");
    client.register_hook("logger".to_string(), logger).await;

    let rate_limiter = Arc::new(RateLimitHook {
        max_calls_per_minute: 10,
    });
    println!("  - rate_limiter → RateLimitHook (enforces rate limits)");
    client
        .register_hook("rate_limiter".to_string(), rate_limiter)
        .await;

    println!();

    // In a real application, you would now:
    // 1. Connect the client: client.connect().await?;
    // 2. Send messages: client.send_message("query").await?;
    // 3. Hooks would be automatically invoked based on matchers
    // 4. Tool calls would be validated by guardrails
    // 5. All events would be logged
    // 6. Rate limits would be enforced

    println!("=== Hook Workflow ===");
    println!("When Claude attempts to use tools:");
    println!("  1. PreToolUse event is triggered");
    println!("  2. Hook matchers determine which hooks to invoke");
    println!("  3. Guardrail hook validates the tool input");
    println!("  4. If approved, tool executes; if blocked, error returned");
    println!("  5. Logger hook records the event");
    println!("  6. Rate limiter checks usage limits");
    println!();

    println!("=== Hook Event Types ===");
    println!("Available hook events:");
    println!("  - PreToolUse: Before a tool is used");
    println!("  - PostToolUse: After a tool successfully completes");
    println!("  - PostToolUseFailure: After a tool fails");
    println!("  - UserPromptSubmit: When user submits a prompt");
    println!("  - Stop: When session stops");
    println!("  - SubagentStart: When a subagent starts");
    println!("  - SubagentStop: When a subagent stops");
    println!("  - PreCompact: Before conversation compaction");
    println!("  - Notification: System notifications");
    println!("  - PermissionRequest: Permission requests");
    println!();

    println!("=== Example Complete ===");
    println!("This example demonstrated:");
    println!("  - Implementing HookHandler trait for custom hooks");
    println!("  - Configuring HookMatcher for tool-specific hooks");
    println!("  - Registering hooks with ClaudeClient");
    println!("  - Validation logic (guardrails) to block dangerous operations");
    println!("  - Logging logic for monitoring and debugging");
    println!("  - Rate limiting patterns");

    Ok(())
}

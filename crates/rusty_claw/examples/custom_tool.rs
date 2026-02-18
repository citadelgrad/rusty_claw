//! Custom tool example demonstrating tool creation and registration.
//!
//! This example shows how to:
//! - Create custom tools using the `#[claw_tool]` proc macro
//! - Handle different parameter types (String, i32, Option<T>)
//! - Register tools with `SdkMcpServerImpl`
//! - Set up an MCP server with multiple tools
//! - Return results using `ToolResult`
//!
//! # Usage
//!
//! ```bash
//! cargo run --example custom_tool --package rusty_claw
//! ```
//!
//! Note: This example demonstrates tool setup but doesn't actually use the tools.
//! To use these tools, they would need to be registered with a ClaudeClient
//! using `client.register_mcp_message_handler(registry)`.

use rusty_claw::claw_tool;
use rusty_claw::prelude::*;

// Tool 1: Calculator - Simple math operations
// Demonstrates: i32 parameters, basic arithmetic
#[claw_tool(
    name = "calculator",
    description = "Perform basic arithmetic operations (add or multiply)"
)]
async fn calculator(operation: String, a: i32, b: i32) -> ToolResult {
    match operation.as_str() {
        "add" => ToolResult::text(format!("Result: {}", a + b)),
        "multiply" => ToolResult::text(format!("Result: {}", a * b)),
        _ => ToolResult::error(format!("Unknown operation: {}", operation)),
    }
}

// Tool 2: Text formatter - String manipulation
// Demonstrates: String parameters, Option<T> for optional params
#[claw_tool(
    name = "format-text",
    description = "Format text with optional prefix and suffix"
)]
async fn format_text(text: String, prefix: Option<String>, suffix: Option<String>) -> ToolResult {
    let mut result = text;

    // Add prefix if provided
    if let Some(p) = prefix {
        result = format!("{}{}", p, result);
    }

    // Add suffix if provided
    if let Some(s) = suffix {
        result = format!("{}{}", result, s);
    }

    ToolResult::text(result)
}

// Tool 3: Echo - Simple echo with optional repeat
// Demonstrates: Option<i32>, default values
#[claw_tool(
    name = "echo",
    description = "Echo text back, optionally repeated multiple times"
)]
async fn echo(text: String, repeat: Option<i32>) -> ToolResult {
    let times = repeat.unwrap_or(1); // Default to 1 if not provided

    if times <= 0 {
        return Ok(ToolResult::error(
            "Repeat count must be positive".to_string(),
        ));
    }

    if times > 100 {
        return Ok(ToolResult::error(
            "Repeat count too large (max 100)".to_string(),
        ));
    }

    let result = (0..times)
        .map(|_| text.clone())
        .collect::<Vec<_>>()
        .join("\n");

    ToolResult::text(result)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Rusty Claw Custom Tool Example ===\n");

    // Create an MCP server to host our custom tools
    println!("Creating MCP server with custom tools...");
    let mut server = SdkMcpServerImpl::new("example-tools", "1.0.0");

    // Register all tools with the server
    // The #[claw_tool] macro generates functions that return SdkMcpTool instances
    println!("Registering tools:");

    let calc_tool = calculator();
    println!("  - {} ({})", calc_tool.name, calc_tool.description);
    server.register_tool(calc_tool);

    let format_tool = format_text();
    println!("  - {} ({})", format_tool.name, format_tool.description);
    server.register_tool(format_tool);

    let echo_tool = echo();
    println!("  - {} ({})", echo_tool.name, echo_tool.description);
    server.register_tool(echo_tool);

    println!("\n=== Server Configuration ===");
    println!("Server name: example-tools");
    println!("Server version: 1.0.0");
    println!("Total tools registered: 3");

    // In a real application, you would register this server with a ClaudeClient:
    println!("\n=== Usage Pattern ===");
    println!("To use these tools with a ClaudeClient:");
    println!("  1. Create a registry: let registry = Arc::new(SdkMcpServerRegistry::new());");
    println!("  2. Register server: registry.register_server(\"example-tools\", server).await;");
    println!("  3. Register with client: client.register_mcp_message_handler(registry).await;");
    println!("  4. Claude can now use these tools when responding to queries");

    // Demonstrate tool schemas
    println!("\n=== Tool Schemas ===");
    println!("calculator:");
    println!("  Parameters:");
    println!("    - operation: String (required) - 'add' or 'multiply'");
    println!("    - a: i32 (required) - first number");
    println!("    - b: i32 (required) - second number");
    println!();

    println!("format-text:");
    println!("  Parameters:");
    println!("    - text: String (required) - text to format");
    println!("    - prefix: Option<String> - optional prefix");
    println!("    - suffix: Option<String> - optional suffix");
    println!();

    println!("echo:");
    println!("  Parameters:");
    println!("    - text: String (required) - text to echo");
    println!("    - repeat: Option<i32> - times to repeat (default: 1, max: 100)");
    println!();

    println!("=== Example Complete ===");
    println!("This example demonstrated:");
    println!("  - Creating tools with #[claw_tool] macro");
    println!("  - Different parameter types (String, i32, Option<T>)");
    println!("  - Registering tools with SdkMcpServerImpl");
    println!("  - Error handling in tool implementations");
    println!("  - Tool naming and description conventions");

    Ok(())
}

//! Advanced tools — showcase `#[claw_tool]` macro features.
//!
//! This example demonstrates advanced features of the `#[claw_tool]` attribute macro:
//! - `Vec<T>` parameters — arrays in the generated JSON Schema
//! - `bool` parameters — boolean flags
//! - Doc comments — used as the tool description when `description` is omitted
//! - Name inference — the function name is used as the tool name when `name` is omitted
//!
//! ## Run
//! ```sh
//! cargo run -p examples --example advanced_tools
//! ```

use rusty_claw::claw_tool;
use rusty_claw::mcp_server::{SdkMcpServerImpl, SdkMcpServerRegistry, SdkMcpTool, ToolResult};
use rusty_claw::prelude::*;
use std::io::{self, Write};
use std::sync::Arc;
use tokio_stream::StreamExt;

// --- Tool 1: Vec<T> parameter ---
// Arrays become {"type": "array", "items": {"type": "string"}} in the schema.

#[claw_tool(
    name = "join_words",
    description = "Join a list of words with a separator"
)]
async fn join_words(words: Vec<String>, separator: Option<String>) -> ToolResult {
    let sep = separator.unwrap_or_else(|| ", ".to_string());
    let joined = words.join(&sep);
    ToolResult::text(format!("Joined: {}", joined))
}

// --- Tool 2: bool parameter ---
// Booleans become {"type": "boolean"} in the schema.

#[claw_tool(
    name = "format_list",
    description = "Format items as a numbered or bulleted list"
)]
async fn format_list(items: Vec<String>, numbered: bool) -> ToolResult {
    let formatted: Vec<String> = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            if numbered {
                format!("{}. {}", i + 1, item)
            } else {
                format!("- {}", item)
            }
        })
        .collect();

    ToolResult::text(formatted.join("\n"))
}

// --- Tool 3: Name inference + doc comment description ---
// When `name` is omitted, the function name becomes the tool name.
// When `description` is omitted, the first doc comment line is used.

/// Calculate the sum of a list of numbers.
#[claw_tool]
async fn sum_numbers(numbers: Vec<f64>) -> ToolResult {
    let total: f64 = numbers.iter().sum();
    ToolResult::text(format!("Sum: {}", total))
}

// --- Tool 4: Complex types ---

#[claw_tool(
    name = "analyze_text",
    description = "Analyze text and return statistics"
)]
async fn analyze_text(text: String, include_words: bool) -> ToolResult {
    let char_count = text.len();
    let word_count = text.split_whitespace().count();
    let line_count = text.lines().count();

    let mut result = format!(
        "Characters: {}\nWords: {}\nLines: {}",
        char_count, word_count, line_count
    );

    if include_words {
        let words: Vec<&str> = text.split_whitespace().collect();
        result.push_str(&format!("\nWord list: {:?}", words));
    }

    ToolResult::text(result)
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

    println!("=== Advanced #[claw_tool] Features ===\n");

    // Build tools using the generated builder functions
    let tools: Vec<SdkMcpTool> = vec![join_words(), format_list(), sum_numbers(), analyze_text()];

    // Display tool schemas
    for tool in &tools {
        println!("Tool: {}", tool.name);
        println!("  Description: {}", tool.description);
        println!(
            "  Schema: {}",
            serde_json::to_string_pretty(&tool.input_schema).unwrap()
        );
        println!();
    }

    // Register all tools with an MCP server
    let mut server = SdkMcpServerImpl::new("advanced_tools", "1.0.0");
    for tool in tools {
        server.register_tool(tool);
    }
    let mut registry = SdkMcpServerRegistry::new();
    registry.register(server);

    println!("Registered tools.\n");

    // Connect and ask Claude to use the tools
    let options = ClaudeAgentOptions::builder()
        .max_turns(5)
        .model("claude-haiku-4-5")
        .permission_mode(PermissionMode::AcceptEdits)
        .sdk_mcp_servers(vec![rusty_claw::options::SdkMcpServer {
            name: "advanced_tools".to_string(),
            version: "1.0.0".to_string(),
        }])
        .build();

    let mut client = ClaudeClient::new(options)?;
    client
        .register_mcp_message_handler(Arc::new(registry))
        .await;
    client.connect().await?;

    let prompt = "Use the advanced_tools to: \
        1) join the words ['Rust', 'is', 'great'] with spaces, \
        2) format ['apples', 'bananas', 'cherries'] as a numbered list, \
        3) sum the numbers [1.5, 2.5, 3.0, 4.0]. \
        Call each tool once.";

    println!("Prompt: {:?}\n", prompt);

    let mut stream = client.send_message(prompt).await?;

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
    println!("\nDone — demonstrated Vec<T>, bool, doc comments, and name inference.");
    Ok(())
}

//! Custom MCP tools — expose Rust functions that Claude can call.
//!
//! The `#[claw_tool]` macro turns a plain `async fn` into an MCP tool.
//! You define the function, the macro generates a handler struct and a
//! builder function, and you register the tool with an MCP server. When
//! Claude decides to call your tool, the SDK routes the JSON-RPC request
//! to your function and sends the result back.
//!
//! ## What happens under the hood
//!
//! 1. `#[claw_tool]` generates a `ToolHandler` impl and a builder fn
//!    (e.g. `word_count()` returns an `SdkMcpTool`). It auto-generates
//!    JSON Schema from the function signature — `Option<T>` params become
//!    optional, everything else is required.
//! 2. `SdkMcpServerImpl` holds your tools. `SdkMcpServerRegistry` routes
//!    incoming JSON-RPC messages (`tools/list`, `tools/call`) to the right
//!    server.
//! 3. `register_mcp_message_handler()` plugs the registry into the client.
//!    When the CLI sends an MCP message, the background router dispatches
//!    it to your registry, which calls your handler, and the result goes
//!    back to Claude.
//!
//! ## Run
//!
//! ```sh
//! cargo run -p examples --example custom_tool
//!
//! # With debug logging
//! RUST_LOG=debug cargo run -p examples --example custom_tool
//! ```

use std::io::{self, Write};

use rusty_claw::claw_tool;
use rusty_claw::mcp_server::{SdkMcpServerImpl, SdkMcpServerRegistry, ToolResult};
use rusty_claw::prelude::*;
use std::sync::Arc;
use tokio_stream::StreamExt;

// Define a tool using the #[claw_tool] macro.
// The macro generates:
//   - A handler struct (WordCountHandler) implementing ToolHandler
//   - A builder function (word_count()) that returns SdkMcpTool
#[claw_tool(name = "word_count", description = "Count words in a text string")]
async fn word_count(text: String) -> ToolResult {
    let count = text.split_whitespace().count();
    ToolResult::text(format!("{} words", count))
}

// Optional parameters use Option<T> — they won't appear in the JSON Schema's
// "required" array, so Claude can omit them.
#[claw_tool(name = "repeat", description = "Repeat a message N times")]
async fn repeat(message: String, times: Option<i32>) -> ToolResult {
    let n = times.unwrap_or(1) as usize;
    let output = std::iter::repeat_n(message, n)
        .collect::<Vec<_>>()
        .join("\n");
    ToolResult::text(output)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Enable tracing so SDK internals (CLI stderr, MCP routing) are visible.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rusty_claw=warn".parse().unwrap()),
        )
        .with_target(false)
        .init();

    // Step 1: Create an MCP server and register tools.
    // Each #[claw_tool] function becomes a builder — call it to get an SdkMcpTool.
    println!("Registering custom tools...");
    let mut server = SdkMcpServerImpl::new("text_tools", "1.0.0");
    server.register_tool(word_count());
    server.register_tool(repeat());
    println!("  - word_count(text: String)");
    println!("  - repeat(message: String, times: Option<i32>)");

    // Step 2: Wrap the server in a registry (routes MCP calls by server name)
    let mut registry = SdkMcpServerRegistry::new();
    registry.register(server);

    // Step 3: Configure the agent to advertise our SDK MCP server
    let options = ClaudeAgentOptions::builder()
        .max_turns(5)
        .model("claude-haiku-4-5")
        .permission_mode(PermissionMode::BypassPermissions)
        .sdk_mcp_servers(vec![rusty_claw::options::SdkMcpServer {
            name: "text_tools".to_string(),
            version: "1.0.0".to_string(),
        }])
        .build();

    // Step 4: Register MCP handler BEFORE connect (CLI sends mcp_message during init)
    println!("\nConnecting to Claude CLI...");
    let mut client = ClaudeClient::new(options)?;
    client
        .register_mcp_message_handler(Arc::new(registry))
        .await;
    client.connect().await?;
    println!("Connected.\n");

    // Step 5: Ask Claude to use the tools
    let prompt = std::env::args().nth(1).unwrap_or_else(|| {
        "Use the word_count tool on 'hello world foo bar' and the repeat tool on 'hi' 3 times"
            .to_string()
    });
    println!("Sending: {prompt:?}");
    println!("Streaming response:\n");

    let mut stream = client.send_message(&prompt).await?;
    let mut message_count = 0;

    while let Some(result) = stream.next().await {
        message_count += 1;
        match result {
            Ok(Message::Assistant(msg)) => {
                for block in &msg.message.content {
                    match block {
                        ContentBlock::Text { text } => {
                            print!("{}", text);
                            io::stdout().flush()?;
                        }
                        ContentBlock::ToolUse { name, .. } => {
                            println!("\n[calling tool: {}]", name);
                        }
                        _ => {}
                    }
                }
            }
            Ok(Message::Result(ResultMessage::Success { .. })) => break,
            Ok(Message::Result(ResultMessage::Error { error, .. })) => {
                eprintln!("Error: {}", error);
                break;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("Stream error: {}", e);
                break;
            }
        }
    }

    if message_count == 0 {
        eprintln!();
        eprintln!("No messages received. Re-run with logging to diagnose:");
        eprintln!("  RUST_LOG=debug cargo run -p examples --example custom_tool");
    }

    client.close().await?;
    println!("\nDone.");

    Ok(())
}

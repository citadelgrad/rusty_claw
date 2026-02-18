//! External MCP servers — configure external MCP server connections.
//!
//! This example demonstrates the `.mcp_servers()` configuration for connecting
//! to external MCP servers (filesystem, database, API servers, etc.).
//!
//! **Note:** `McpServerConfig` is currently a placeholder struct — the full
//! configuration API (command, args, env, URL) is planned for a future release.
//! This example documents the intended usage pattern.
//!
//! For SDK-hosted MCP tools (Rust functions), see the `custom_tool` example instead.
//!
//! ## Run
//! ```sh
//! cargo run -p examples --example external_mcp
//! ```

use rusty_claw::prelude::*;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rusty_claw=warn".parse().unwrap()),
        )
        .with_target(false)
        .init();

    println!("=== External MCP Server Configuration ===\n");

    // --- Current API (placeholder) ---
    // McpServerConfig is currently an empty struct. The CLI's --mcp-config
    // flag expects a JSON config file, but the SDK doesn't yet populate it
    // with server connection details.

    let mut mcp_servers = HashMap::new();

    // TODO: Once McpServerConfig is fully implemented, these will include:
    //   command: "npx",
    //   args: ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"],
    //   env: { "KEY": "value" },
    //   url: "http://localhost:3000/mcp",
    use rusty_claw::options::McpServerConfig;

    mcp_servers.insert("filesystem".to_string(), McpServerConfig {});
    mcp_servers.insert("database".to_string(), McpServerConfig {});

    let options = ClaudeAgentOptions::builder()
        .max_turns(3)
        .permission_mode(PermissionMode::AcceptEdits)
        .mcp_servers(mcp_servers)
        .build();

    println!(
        "Configured {} external MCP server(s):",
        options.mcp_servers.len()
    );
    for name in options.mcp_servers.keys() {
        println!("  - {}", name);
    }

    // --- SDK-hosted MCP tools (working API) ---
    println!("\n=== SDK-Hosted MCP Comparison ===\n");
    println!("For Rust-native tools, use SdkMcpServerImpl instead:");
    println!();
    println!("  let mut server = SdkMcpServerImpl::new(\"my_tools\", \"1.0.0\");");
    println!("  server.register_tool(my_tool);");
    println!("  let registry = SdkMcpServerRegistry::new(vec![server]);");
    println!("  client.register_mcp_message_handler(Arc::new(registry));");
    println!();
    println!("See the `custom_tool` example for a complete working demo.\n");

    // --- Intended future usage ---
    println!("=== Intended Future API ===\n");
    println!("When McpServerConfig is fully implemented:");
    println!();
    println!("  let mut servers = HashMap::new();");
    println!("  servers.insert(\"filesystem\".to_string(), McpServerConfig {{");
    println!("      command: \"npx\".to_string(),");
    println!("      args: vec![\"-y\", \"@modelcontextprotocol/server-filesystem\", \"/tmp\"],");
    println!("      env: HashMap::new(),");
    println!("  }});");
    println!();
    println!("  let options = ClaudeAgentOptions::builder()");
    println!("      .mcp_servers(servers)");
    println!("      .build();");
    println!();

    println!("Done — documented external MCP server configuration.");
    Ok(())
}

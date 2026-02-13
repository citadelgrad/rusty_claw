//! Example demonstrating subagent support in Rusty Claw
//!
//! This example shows how to:
//! - Define agent configurations with prompts and tool restrictions
//! - Register SubagentStart and SubagentStop hooks
//! - Initialize a ClaudeClient with multiple agents
//!
//! # Usage
//!
//! ```bash
//! cargo run --example subagent_usage --package rusty_claw
//! ```

use rusty_claw::options::AgentDefinition;
use rusty_claw::prelude::*;
use std::collections::HashMap;

#[tokio::main]
async fn main() {
    println!("=== Rusty Claw Subagent Example ===\n");

    // Define a researcher agent
    let researcher = AgentDefinition {
        description: "Research agent specialized in code analysis".to_string(),
        prompt: "You are a research assistant focused on analyzing code and finding patterns."
            .to_string(),
        tools: vec![
            "Read".to_string(),
            "Grep".to_string(),
            "Glob".to_string(),
        ],
        model: Some("claude-sonnet-4".to_string()),
    };

    // Define a writer agent
    let writer = AgentDefinition {
        description: "Writing agent specialized in documentation".to_string(),
        prompt: "You are a technical writer focused on creating clear documentation.".to_string(),
        tools: vec![
            "Read".to_string(),
            "Write".to_string(),
            "Edit".to_string(),
        ],
        model: None, // Uses default model
    };

    // Collect agents into a HashMap
    let mut agents = HashMap::new();
    agents.insert("researcher".to_string(), researcher);
    agents.insert("writer".to_string(), writer);

    println!("Configured {} agents:", agents.len());
    for (name, agent) in &agents {
        println!(
            "  - {}: {} (tools: {})",
            name,
            agent.description,
            agent.tools.join(", ")
        );
    }
    println!();

    // Register hooks for subagent lifecycle events
    let mut hooks = HashMap::new();

    // SubagentStart hook - matches Bash tool when subagent starts
    let subagent_start_hook = HookMatcher {
        tool_name: Some("Bash".to_string()),
    };
    hooks.insert(
        HookEvent::SubagentStart,
        vec![subagent_start_hook],
    );

    // SubagentStop hook - matches Bash tool when subagent stops
    let subagent_stop_hook = HookMatcher {
        tool_name: Some("Bash".to_string()),
    };
    hooks.insert(
        HookEvent::SubagentStop,
        vec![subagent_stop_hook],
    );

    println!("Registered {} hook events:", hooks.len());
    for (event, matchers) in &hooks {
        println!("  - {:?}: {} matcher(s)", event, matchers.len());
    }
    println!();

    // Build options with agents and hooks
    let options = ClaudeAgentOptions::builder()
        .agents(agents.clone())
        .hooks(hooks.clone())
        .build();

    println!("ClaudeAgentOptions configured:");
    println!("  - Agents: {} defined", options.agents.len());
    println!("  - Hooks: {} events registered", options.hooks.len());
    println!();

    // In a real application, you would initialize a ClaudeClient with these options:
    //
    // let transport = SubprocessCLITransport::default();
    // let mut client = ClaudeClient::new(transport, options);
    // client.initialize().await?;
    //
    // Then spawn subagents using the defined agent names:
    //
    // client.spawn_agent("researcher").await?;
    // client.spawn_agent("writer").await?;

    println!("Example completed successfully!");
    println!();
    println!("Key Takeaways:");
    println!("  1. AgentDefinition specifies prompts, tools, and optional model overrides");
    println!("  2. SubagentStart/SubagentStop hooks track agent lifecycle");
    println!("  3. Agents are registered in Initialize control request");
    println!("  4. Tool restrictions limit what each agent can access");
}

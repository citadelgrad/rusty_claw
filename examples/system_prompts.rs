//! System prompts — customize Claude's behavior with custom or preset prompts.
//!
//! This example demonstrates three ways to set the system prompt:
//! - `SystemPrompt::Custom(...)` — provide your own system prompt text
//! - `SystemPrompt::Preset { preset }` — use a named CLI preset
//! - `.append_system_prompt(...)` — add text to the default system prompt
//!
//! ## Run
//! ```sh
//! cargo run -p examples --example system_prompts
//! ```

use std::io::{self, Write};

use rusty_claw::prelude::*;
use tokio_stream::StreamExt;

/// Run a one-shot query with the given options and print the response.
async fn run_query(
    label: &str,
    options: ClaudeAgentOptions,
    prompt: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("--- {} ---", label);
    println!("Prompt: {:?}\n", prompt);

    let mut stream = rusty_claw::query(prompt, Some(options)).await?;

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
    println!("\n");
    Ok(())
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

    println!("=== System Prompt Configurations ===\n");

    // 1. Custom system prompt — full replacement
    let custom_options = ClaudeAgentOptions::builder()
        .max_turns(1)
        .model("claude-haiku-4-5")
        .permission_mode(PermissionMode::AcceptEdits)
        .system_prompt(SystemPrompt::Custom(
            "You are a pirate. Respond to everything in pirate speak. \
             Keep responses to one sentence."
                .to_string(),
        ))
        .build();

    run_query(
        "Custom System Prompt (pirate mode)",
        custom_options,
        "What is Rust?",
    )
    .await?;

    // 2. Preset system prompt — use a named CLI preset
    let preset_options = ClaudeAgentOptions::builder()
        .max_turns(1)
        .model("claude-haiku-4-5")
        .permission_mode(PermissionMode::AcceptEdits)
        .system_prompt(SystemPrompt::Preset {
            preset: "default".to_string(),
        })
        .build();

    run_query(
        "Preset System Prompt (default)",
        preset_options,
        "What is Rust? Answer in one sentence.",
    )
    .await?;

    // 3. Append to system prompt — add context without replacing
    let append_options = ClaudeAgentOptions::builder()
        .max_turns(1)
        .model("claude-haiku-4-5")
        .permission_mode(PermissionMode::AcceptEdits)
        .append_system_prompt("IMPORTANT: Always end your response with '-- Rusty Claw SDK'")
        .build();

    run_query(
        "Append System Prompt",
        append_options,
        "What is Rust? Answer in one sentence.",
    )
    .await?;

    // Show the CLI args that each configuration generates
    println!("=== CLI Argument Comparison ===\n");

    let custom = ClaudeAgentOptions::builder()
        .system_prompt(SystemPrompt::Custom("You are a pirate.".into()))
        .build();
    println!(
        "Custom: {:?}\n",
        custom
            .to_cli_args("test")
            .iter()
            .filter(|a| a.contains("system-prompt") || a.starts_with("--system"))
            .collect::<Vec<_>>()
    );

    let preset = ClaudeAgentOptions::builder()
        .system_prompt(SystemPrompt::Preset {
            preset: "default".into(),
        })
        .build();
    println!(
        "Preset: {:?}\n",
        preset
            .to_cli_args("test")
            .iter()
            .filter(|a| a.contains("system-prompt") || a.starts_with("--system"))
            .collect::<Vec<_>>()
    );

    let append = ClaudeAgentOptions::builder()
        .append_system_prompt("Extra context here.")
        .build();
    println!(
        "Append: {:?}\n",
        append
            .to_cli_args("test")
            .iter()
            .filter(|a| a.contains("system-prompt") || a.starts_with("--append"))
            .collect::<Vec<_>>()
    );

    println!("Done — demonstrated custom, preset, and append system prompts.");
    Ok(())
}

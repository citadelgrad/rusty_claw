//! Advanced configuration — settings sources, output format, and betas.
//!
//! This example demonstrates lesser-used configuration options:
//! - `.settings_sources()` — control which settings files the CLI reads
//! - `.output_format()` — customize CLI output format
//! - `.betas()` — enable beta/experimental features
//!
//! ## Run
//! ```sh
//! cargo run -p examples --example advanced_config
//! ```

use rusty_claw::prelude::*;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rusty_claw=warn".parse().unwrap()),
        )
        .with_target(false)
        .init();

    println!("=== Advanced Configuration ===\n");

    // --- 1. Settings Sources ---
    // Control which settings files the CLI reads. By default, the SDK passes
    // an empty list to prevent user-level settings from affecting agent behavior.
    // You can override this to include specific sources.

    println!("--- Settings Sources ---\n");

    // Default: no settings (isolated agent)
    let isolated = ClaudeAgentOptions::builder().max_turns(3).build();
    println!("Default settings_sources: {:?}", isolated.settings_sources);

    // Custom: only read local project settings
    let local_only = ClaudeAgentOptions::builder()
        .max_turns(3)
        .settings_sources(vec!["local".to_string(), "project".to_string()])
        .build();
    println!("Custom settings_sources: {:?}", local_only.settings_sources);

    // Show how it affects CLI args
    let args = local_only.to_cli_args("test");
    let settings_args: Vec<_> = args.iter().filter(|a| a.contains("setting")).collect();
    println!("CLI args: {:?}\n", settings_args);

    // --- 2. Output Format ---
    // The SDK normally sets output_format to "stream-json" for NDJSON streaming.
    // You can provide a custom format configuration if needed.

    println!("--- Output Format ---\n");

    let custom_format = ClaudeAgentOptions::builder()
        .max_turns(3)
        .output_format(json!({
            "type": "stream-json",
            "include_metadata": true
        }))
        .build();
    println!("Custom output_format: {:?}", custom_format.output_format);

    // Default (None) — the SDK sets stream-json automatically
    let default_format = ClaudeAgentOptions::builder().max_turns(3).build();
    println!(
        "Default output_format: {:?}\n",
        default_format.output_format
    );

    // --- 3. Betas ---
    // Enable experimental CLI features by name.
    // These are passed as --beta flags to the CLI.

    println!("--- Beta Features ---\n");

    let with_betas = ClaudeAgentOptions::builder()
        .max_turns(3)
        .betas(vec![
            "extended-thinking".to_string(),
            "tool-streaming".to_string(),
        ])
        .build();
    println!("Enabled betas: {:?}", with_betas.betas);

    let args = with_betas.to_cli_args("test");
    let beta_args: Vec<_> = args.iter().filter(|a| a.contains("beta")).collect();
    println!("CLI args: {:?}\n", beta_args);

    // --- 4. Combined advanced configuration ---
    println!("--- Combined Configuration ---\n");

    let advanced = ClaudeAgentOptions::builder()
        .max_turns(5)
        .model("claude-sonnet-4-5")
        .permission_mode(PermissionMode::AcceptEdits)
        .settings_sources(vec!["local".to_string()])
        .betas(vec!["extended-thinking".to_string()])
        .allowed_tools(vec!["Read".to_string(), "Grep".to_string()])
        .build();

    println!("Combined options:");
    println!("  model:            {:?}", advanced.model);
    println!("  max_turns:        {:?}", advanced.max_turns);
    println!("  permission_mode:  {:?}", advanced.permission_mode);
    println!("  settings_sources: {:?}", advanced.settings_sources);
    println!("  betas:            {:?}", advanced.betas);
    println!("  allowed_tools:    {:?}", advanced.allowed_tools);
    println!();

    let all_args = advanced.to_cli_args("test prompt");
    println!("Full CLI args:");
    for arg in &all_args {
        println!("  {}", arg);
    }

    println!("\nDone — demonstrated settings_sources, output_format, and betas.");
    Ok(())
}

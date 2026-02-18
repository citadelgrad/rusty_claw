//! Rusty Claw - Rust implementation of the Claude Agent SDK
//!
//! This crate provides a Rust implementation of the Claude Agent SDK, architecturally
//! inspired by Anthropic's Python SDK ([claude-agent-sdk-python](https://github.com/anthropics/claude-agent-sdk-python))
//! licensed under MIT.
//!
//! # Overview
//!
//! Rusty Claw enables building Claude-powered agents in Rust with support for:
//! - Bidirectional JSONL transport over stdio
//! - Claude Control Protocol (CCP) message handling
//! - Model Context Protocol (MCP) tool integration
//! - Hook system for lifecycle events
//! - Procedural macros for ergonomic tool definitions
//!
//! # Architecture
//!
//! The SDK is organized into several key modules:
//! - `transport`: Low-level JSONL message transport over stdio
//! - `control`: Claude Control Protocol implementation
//! - `mcp`: Model Context Protocol integration
//! - `hooks`: Lifecycle event hooks
//! - `error`: Error types and handling
//!
//! # Subagent Support
//!
//! Rusty Claw supports spawning and managing subagents with dedicated prompts and tool restrictions:
//!
//! ```no_run
//! use rusty_claw::prelude::*;
//! use rusty_claw::options::AgentDefinition;
//! use std::collections::HashMap;
//!
//! # async fn example() -> Result<(), ClawError> {
//! // Define specialized agents
//! let mut agents = HashMap::new();
//! agents.insert(
//!     "researcher".to_string(),
//!     AgentDefinition {
//!         description: "Research agent for code analysis".to_string(),
//!         prompt: "You are a research assistant".to_string(),
//!         tools: vec!["Read".to_string(), "Grep".to_string()],
//!         model: Some("claude-sonnet-4".to_string()),
//!     },
//! );
//!
//! // Configure lifecycle hooks
//! let mut hooks = HashMap::new();
//! hooks.insert(
//!     HookEvent::SubagentStart,
//!     vec![HookMatcher { tool_name: Some("Bash".to_string()) }],
//! );
//!
//! // Build options and connect client
//! let options = ClaudeAgentOptions::builder()
//!     .agents(agents)
//!     .hooks(hooks)
//!     .build();
//!
//! let mut client = ClaudeClient::new(options)?;
//! client.connect().await?;
//! # Ok(())
//! # }
//! ```
//!
//! See [`examples/subagent_usage.rs`](https://github.com/citadelgrad/rusty_claw/blob/main/crates/rusty_claw/examples/subagent_usage.rs)
//! for a complete working example.
//!
//! For detailed documentation on subagent lifecycle hooks, see [`docs/HOOKS.md`](https://github.com/citadelgrad/rusty_claw/blob/main/docs/HOOKS.md).
//!
//! # Example
//!
//! ```rust,no_run
//! use rusty_claw::prelude::*;
//! use tokio_stream::StreamExt;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), ClawError> {
//!     let options = ClaudeAgentOptions::builder()
//!         .allowed_tools(vec!["Read".into(), "Edit".into(), "Bash".into()])
//!         .permission_mode(PermissionMode::AcceptEdits)
//!         .build();
//!
//!     let mut stream = query("Find and fix the bug in auth.py", Some(options)).await?;
//!
//!     while let Some(message) = stream.next().await {
//!         match message? {
//!             Message::Assistant(msg) => {
//!                 for block in msg.message.content {
//!                     if let ContentBlock::Text { text } = block {
//!                         println!("{}", text);
//!                     }
//!                 }
//!             }
//!             Message::Result(ResultMessage::Success { result, .. }) => {
//!                 println!("Done: {}", result);
//!             }
//!             _ => {}
//!         }
//!     }
//!     Ok(())
//! }
//! ```
//!
//! # License
//!
//! Licensed under MIT. See LICENSE file for details.

#![warn(missing_docs)]
#![warn(clippy::all)]

// Re-export macros from rusty_claw_macros
pub use rusty_claw_macros::*;

// Module structure - to be implemented in future tasks

/// Low-level transport layer for JSONL communication over stdio
///
/// This module provides the `Transport` trait, which defines an async interface for
/// bidirectional JSONL message exchange with the Claude Code CLI over stdin/stdout.
///
/// `SubprocessCLITransport` is the default implementation that spawns the `claude` CLI
/// as a subprocess and manages its lifecycle.
///
/// The `Transport` trait can be implemented for custom transports (e.g., remote connections,
/// testing mock transports, alternative CLIs).
pub mod transport;

/// Claude Control Protocol (CCP) implementation
///
/// This module implements the Claude Control Protocol for bidirectional communication between
/// the agent and the Claude CLI. It handles control requests like permission checks, tool
/// availability queries, and session lifecycle management.
pub mod control;

/// Model Context Protocol (MCP) integration
///
/// This module provides the integration layer for MCP tools, allowing agents to expose
/// custom tools to the Claude CLI. See `SdkMcpServerImpl` for the main server implementation.
pub mod mcp_server;

/// Hook system for lifecycle events
///
/// The hook system allows agents to intercept and respond to lifecycle events such as
/// tool invocations, permission requests, and subagent operations.
///
/// Key types:
/// - `HookEvent` - Events that trigger hooks (defined in options module)
/// - `HookMatcher` - Pattern matching for selective hook triggering (defined in options module)
/// - `HookCallback` - Trait for implementing hook logic
/// - `HookInput` - Data passed to hooks
/// - `HookContext` - Session context available to hooks
/// - `HookResponse` - Response with permission decisions
pub mod hooks;

/// Permission management for tool usage control
///
/// This module provides permission handlers for controlling tool access and usage
/// in Claude agents. See `DefaultPermissionHandler` for the default implementation.
pub mod permissions;

/// Error types and utilities
///
/// This module defines the `ClawError` enum, which covers all error cases in the SDK:
///
/// - `CliNotFound` - Claude Code CLI binary not found during discovery
/// - `InvalidCliVersion` - CLI version is older than required (< 2.0.0)
/// - `Connection` - Transport connection failures
/// - `Process` - CLI process crashes or non-zero exits
/// - `JsonDecode` - JSONL parsing errors (auto-converts from `serde_json::Error`)
/// - `MessageParse` - Malformed control protocol messages
/// - `ControlTimeout` - Control protocol request timeouts
/// - `ControlError` - Control protocol semantic errors
/// - `Io` - Filesystem and I/O operations (auto-converts from `std::io::Error`)
/// - `ToolExecution` - MCP tool handler failures
pub mod error;

/// Message types and structures
///
/// This module defines all message types that flow between the agent and Claude CLI.
///
/// The primary `Message` enum represents all possible messages from the CLI:
///
/// - `System` - System lifecycle events (init, compact boundary)
/// - `Assistant` - Assistant responses with content blocks
/// - `User` - User input messages
/// - `Result` - Final results (success, error, input required)
///
/// Assistant messages contain `ContentBlock` items:
///
/// - `Text` - Plain text content
/// - `ToolUse` - Tool invocation requests
/// - `ToolResult` - Tool execution results
/// - `Thinking` - Extended thinking tokens
pub mod messages;

/// Simple query API for one-shot Claude interactions
///
/// This module provides the `query()` function for simple, one-shot queries to Claude
/// without managing a persistent client connection.
pub mod query;

/// Configuration options and builder
///
/// This module provides `ClaudeAgentOptions` for configuring Claude agent behavior,
/// including model selection, permission modes, hook configurations, and agent definitions.
pub mod options;

/// Client for interactive sessions with Claude CLI
///
/// This module provides `ClaudeClient`, a persistent client for multi-turn
/// conversations with the Claude Code CLI. Unlike the one-shot `query()` API, `ClaudeClient`
/// maintains state across multiple messages and supports control operations like model switching
/// and session interruption.
pub mod client;

// Public API re-exports
pub use query::query;

// Prelude module for common imports
pub mod prelude {
    //! Common imports for rusty_claw users
    //!
    //! Use `use rusty_claw::prelude::*;` to import commonly used types.

    pub use crate::client::{ClaudeClient, ResponseStream};
    pub use crate::control::handlers::{CanUseToolHandler, HookHandler, McpMessageHandler};
    pub use crate::control::messages::{ControlRequest, ControlResponse, IncomingControlRequest};
    pub use crate::control::ControlProtocol;
    pub use crate::error::ClawError;
    pub use crate::hooks::{
        HookCallback, HookContext, HookInput, HookResponse, PermissionDecision,
    };
    pub use crate::mcp_server::{
        SdkMcpServerImpl, SdkMcpServerRegistry, SdkMcpTool, ToolContent, ToolHandler, ToolResult,
    };
    pub use crate::messages::{
        ApiMessage, AssistantMessage, ContentBlock, McpServerInfo, Message, ResultMessage,
        StreamEvent, SystemMessage, ToolInfo, UsageInfo, UserMessage,
    };
    pub use crate::options::{
        ClaudeAgentOptions, HookEvent, HookMatcher, PermissionMode, SystemPrompt,
    };
    pub use crate::permissions::DefaultPermissionHandler;
    pub use crate::query::query;
    pub use crate::transport::{CliDiscovery, SubprocessCLITransport, Transport};
}

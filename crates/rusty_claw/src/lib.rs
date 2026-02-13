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
//! ```ignore
//! // Coming soon: example of creating a simple agent
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
/// This module provides the [Transport](crate::transport::Transport) trait, which defines an async interface for
/// bidirectional JSONL message exchange with the Claude Code CLI over stdin/stdout.
///
/// [SubprocessCLITransport](crate::transport::SubprocessCLITransport) is the default implementation that spawns the `claude` CLI
/// as a subprocess and manages its lifecycle.
///
/// The [Transport](crate::transport::Transport) trait can be implemented for custom transports (e.g., remote connections,
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
/// custom tools to the Claude CLI. See [SdkMcpServerImpl](crate::mcp_server::SdkMcpServerImpl) for the main server implementation.
pub mod mcp_server;

/// Hook system for lifecycle events
///
/// The hook system allows agents to intercept and respond to lifecycle events such as
/// tool invocations, permission requests, and subagent operations.
///
/// Key types:
/// - [HookEvent](crate::options::HookEvent) - Events that trigger hooks (defined in options module)
/// - [HookMatcher](crate::options::HookMatcher) - Pattern matching for selective hook triggering (defined in options module)
/// - [HookCallback](crate::hooks::HookCallback) - Trait for implementing hook logic
/// - [HookInput](crate::hooks::HookInput) - Data passed to hooks
/// - [HookContext](crate::hooks::HookContext) - Session context available to hooks
/// - [HookResponse](crate::hooks::HookResponse) - Response with permission decisions
pub mod hooks;

/// Permission management for tool usage control
///
/// This module provides permission handlers for controlling tool access and usage
/// in Claude agents. See [DefaultPermissionHandler](crate::permissions::DefaultPermissionHandler) for the default implementation.
pub mod permissions;

/// Error types and utilities
///
/// This module defines the [ClawError](crate::error::ClawError) enum, which covers all error cases in the SDK:
///
/// - [ClawError::CliNotFound](crate::error::ClawError::CliNotFound) - Claude Code CLI binary not found during discovery
/// - [ClawError::InvalidCliVersion](crate::error::ClawError::InvalidCliVersion) - CLI version is older than required (< 2.0.0)
/// - [ClawError::Connection](crate::error::ClawError::Connection) - Transport connection failures
/// - [ClawError::Process](crate::error::ClawError::Process) - CLI process crashes or non-zero exits
/// - [ClawError::JsonDecode](crate::error::ClawError::JsonDecode) - JSONL parsing errors (auto-converts from `serde_json::Error`)
/// - [ClawError::MessageParse](crate::error::ClawError::MessageParse) - Malformed control protocol messages
/// - [ClawError::ControlTimeout](crate::error::ClawError::ControlTimeout) - Control protocol request timeouts
/// - [ClawError::ControlError](crate::error::ClawError::ControlError) - Control protocol semantic errors
/// - [ClawError::Io](crate::error::ClawError::Io) - Filesystem and I/O operations (auto-converts from `std::io::Error`)
/// - [ClawError::ToolExecution](crate::error::ClawError::ToolExecution) - MCP tool handler failures
pub mod error;

/// Message types and structures
///
/// This module defines all message types that flow between the agent and Claude CLI.
///
/// The primary [Message](crate::messages::Message) enum represents all possible messages from the CLI:
///
/// - [Message::System](crate::messages::Message::System) - System lifecycle events (init, compact boundary)
/// - [Message::Assistant](crate::messages::Message::Assistant) - Assistant responses with content blocks
/// - [Message::User](crate::messages::Message::User) - User input messages
/// - [Message::Result](crate::messages::Message::Result) - Final results (success, error, input required)
///
/// Assistant messages contain [ContentBlock](crate::messages::ContentBlock) items:
///
/// - [ContentBlock::Text](crate::messages::ContentBlock::Text) - Plain text content
/// - [ContentBlock::ToolUse](crate::messages::ContentBlock::ToolUse) - Tool invocation requests
/// - [ContentBlock::ToolResult](crate::messages::ContentBlock::ToolResult) - Tool execution results
/// - [ContentBlock::Thinking](crate::messages::ContentBlock::Thinking) - Extended thinking tokens
pub mod messages;

/// Simple query API for one-shot Claude interactions
///
/// This module provides the [query()](crate::query::query) function for simple, one-shot queries to Claude
/// without managing a persistent client connection.
pub mod query;

/// Configuration options and builder
///
/// This module provides [ClaudeAgentOptions](crate::options::ClaudeAgentOptions) for configuring Claude agent behavior,
/// including model selection, permission modes, hook configurations, and agent definitions.
pub mod options;

/// Client for interactive sessions with Claude CLI
///
/// This module provides [ClaudeClient](crate::client::ClaudeClient), a persistent client for multi-turn
/// conversations with the Claude Code CLI. Unlike the one-shot [query()](crate::query::query) API, [ClaudeClient](crate::client::ClaudeClient)
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
    pub use crate::hooks::{HookCallback, HookContext, HookInput, HookResponse, PermissionDecision};
    pub use crate::mcp_server::{
        SdkMcpServerImpl, SdkMcpServerRegistry, SdkMcpTool, ToolContent, ToolHandler, ToolResult,
    };
    pub use crate::messages::{
        ApiMessage, AssistantMessage, ContentBlock, McpServerInfo, Message, ResultMessage,
        StreamEvent, SystemMessage, ToolInfo, UsageInfo, UserMessage,
    };
    pub use crate::options::{ClaudeAgentOptions, HookEvent, HookMatcher, PermissionMode, SystemPrompt};
    pub use crate::permissions::DefaultPermissionHandler;
    pub use crate::query::query;
    pub use crate::transport::{CliDiscovery, SubprocessCLITransport, Transport};
}

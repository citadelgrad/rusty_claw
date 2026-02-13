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
pub mod transport;

/// Claude Control Protocol (CCP) implementation
pub mod control;

/// Model Context Protocol (MCP) integration
///
/// MCP integration will be added in future tasks
pub mod mcp {}

/// Hook system for lifecycle events
pub mod hooks;

/// Error types and utilities
pub mod error;

/// Message types and structures
pub mod messages;

/// Simple query API for one-shot Claude interactions
pub mod query;

/// Configuration options and builder
pub mod options;

// Public API re-exports
pub use query::query;

// Prelude module for common imports
pub mod prelude {
    //! Common imports for rusty_claw users
    //!
    //! Use `use rusty_claw::prelude::*;` to import commonly used types.

    pub use crate::control::handlers::{CanUseToolHandler, HookHandler, McpMessageHandler};
    pub use crate::control::messages::{ControlRequest, ControlResponse, IncomingControlRequest};
    pub use crate::control::ControlProtocol;
    pub use crate::error::ClawError;
    pub use crate::hooks::{HookCallback, HookContext, HookInput, HookResponse, PermissionDecision};
    pub use crate::messages::{
        ApiMessage, AssistantMessage, ContentBlock, McpServerInfo, Message, ResultMessage,
        StreamEvent, SystemMessage, ToolInfo, UsageInfo, UserMessage,
    };
    pub use crate::options::{ClaudeAgentOptions, HookEvent, HookMatcher, PermissionMode, SystemPrompt};
    pub use crate::query::query;
    pub use crate::transport::{CliDiscovery, SubprocessCLITransport, Transport};
}

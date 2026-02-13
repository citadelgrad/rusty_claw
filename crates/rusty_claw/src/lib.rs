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
pub mod transport {
    //! Transport implementation will be added in future tasks
}

/// Claude Control Protocol (CCP) implementation
pub mod control {
    //! Control protocol implementation will be added in future tasks
}

/// Model Context Protocol (MCP) integration
pub mod mcp {
    //! MCP integration will be added in future tasks
}

/// Hook system for lifecycle events
pub mod hooks {
    //! Hooks implementation will be added in future tasks
}

/// Error types and utilities
pub mod error;

// Prelude module for common imports
pub mod prelude {
    //! Common imports for rusty_claw users
    //!
    //! Use `use rusty_claw::prelude::*;` to import commonly used types.

    pub use crate::error::ClawError;
}

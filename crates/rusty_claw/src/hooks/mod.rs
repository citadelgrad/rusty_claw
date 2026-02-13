//! Hook system for event-driven callbacks and permission management.
//!
//! The hooks module provides a flexible system for responding to lifecycle events in Claude agents.
//! Hooks can intercept tool uses, user prompts, and session events, allowing you to:
//!
//! - Control which tools can be used
//! - Inject additional context into Claude's prompts
//! - Modify tool inputs before execution
//! - Implement custom permission policies
//!
//! # Architecture
//!
//! The hook system consists of several key components:
//!
//! - `HookEvent` - Events that trigger hooks (defined in options module)
//! - `HookMatcher` - Pattern matching for selective hook triggering
//! - `HookCallback` - Trait for implementing hook logic
//! - `HookInput` - Data passed to hooks
//! - `HookContext` - Session context available to hooks
//! - `HookResponse` - Response with permission decisions
//!
//! # Examples
//!
//! ## Basic Hook: Block Dangerous Tools
//!
//! ```
//! use rusty_claw::prelude::*;
//!
//! async fn block_rm(
//!     input: HookInput,
//!     _tool_use_id: Option<&str>,
//!     _context: &HookContext,
//! ) -> Result<HookResponse, ClawError> {
//!     if let Some(tool_name) = &input.tool_name {
//!         if tool_name == "Bash" {
//!             if let Some(tool_input) = &input.tool_input {
//!                 if let Some(cmd) = tool_input.get("command").and_then(|v| v.as_str()) {
//!                     if cmd.contains("rm -rf") {
//!                         return Ok(HookResponse::deny("Destructive command blocked"));
//!                     }
//!                 }
//!             }
//!         }
//!     }
//!     Ok(HookResponse::allow("Safe"))
//! }
//! ```
//!
//! ## Hook with Context Injection
//!
//! ```
//! use rusty_claw::prelude::*;
//!
//! async fn inject_context(
//!     input: HookInput,
//!     _tool_use_id: Option<&str>,
//!     context: &HookContext,
//! ) -> Result<HookResponse, ClawError> {
//!     let additional_context = if let Some(tools) = &context.available_tools {
//!         format!("Available tools: {}", tools.join(", "))
//!     } else {
//!         "No tools available".to_string()
//!     };
//!
//!     Ok(HookResponse::allow("Approved")
//!         .with_context(additional_context))
//! }
//! ```
//!
//! ## Pattern Matching
//!
//! ```
//! use rusty_claw::prelude::*;
//!
//! // Match all tools
//! let matcher = HookMatcher::all();
//! assert!(matcher.matches("Bash"));
//! assert!(matcher.matches("Read"));
//!
//! // Match specific tool
//! let matcher = HookMatcher::tool("Bash");
//! assert!(matcher.matches("Bash"));
//! assert!(!matcher.matches("Read"));
//! ```

mod callback;
mod response;
mod types;

pub use callback::HookCallback;
pub use response::{HookResponse, PermissionDecision};
pub use types::{HookContext, HookInput};

// Re-export HookEvent and HookMatcher from options for convenience
pub use crate::options::{HookEvent, HookMatcher};

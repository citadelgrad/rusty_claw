//! Permission management for tool usage control.
//!
//! This module provides a flexible permission system for controlling which tools
//! an agent can use during execution. It integrates with the Hook system to allow
//! custom permission logic via callbacks.
//!
//! # Architecture
//!
//! The permission system evaluates tool usage requests through multiple layers:
//!
//! 1. **Explicit Deny** - Check disallowed_tools first (highest priority)
//! 2. **Explicit Allow** - Check allowed_tools second
//! 3. **Hook Decision** - Invoke registered hooks for custom logic
//! 4. **Default Policy** - Fall back to PermissionMode setting
//!
//! # Permission Modes
//!
//! The [`PermissionMode`](crate::options::PermissionMode) enum controls default
//! behavior when no explicit allow/deny rules match:
//!
//! - `Allow` - Allow all tools by default
//! - `Ask` - Prompt user for each tool use
//! - `Deny` - Deny all tools by default
//! - `Custom` - Require hook-based decision (error if no hooks)
//! - `Default/AcceptEdits/BypassPermissions/Plan` - Use CLI defaults
//!
//! # Permission Result
//!
//! Handlers return a rich [`PermissionDecision`](crate::permissions::PermissionDecision) enum rather than a simple bool:
//!
//! - [`PermissionDecision::Allow`](crate::permissions::PermissionDecision::Allow) - Allow the tool, optionally with a modified input
//! - [`PermissionDecision::Deny`](crate::permissions::PermissionDecision::Deny) - Deny the tool, optionally interrupting the session
//!
//! # Examples
//!
//! ## Basic Permission Configuration
//!
//! ```rust
//! use rusty_claw::permissions::DefaultPermissionHandler;
//! use rusty_claw::options::PermissionMode;
//!
//! // Allow specific tools only
//! let handler = DefaultPermissionHandler::builder()
//!     .mode(PermissionMode::Deny)
//!     .allowed_tools(vec!["bash".to_string(), "read".to_string()])
//!     .build();
//! ```
//!
//! ## With Deny List
//!
//! ```rust
//! use rusty_claw::permissions::DefaultPermissionHandler;
//! use rusty_claw::options::PermissionMode;
//!
//! // Allow all except dangerous tools
//! let handler = DefaultPermissionHandler::builder()
//!     .mode(PermissionMode::Allow)
//!     .disallowed_tools(vec!["bash".to_string(), "write".to_string()])
//!     .build();
//! ```
//!
//! ## Input Mutation via Custom Handler
//!
//! ```rust
//! use rusty_claw::control::handlers::CanUseToolHandler;
//! use rusty_claw::permissions::PermissionDecision;
//! use rusty_claw::error::ClawError;
//! use async_trait::async_trait;
//! use serde_json::{json, Value};
//!
//! struct SanitizingHandler;
//!
//! #[async_trait]
//! impl CanUseToolHandler for SanitizingHandler {
//!     async fn can_use_tool(
//!         &self,
//!         tool_name: &str,
//!         tool_input: &Value,
//!     ) -> Result<PermissionDecision, ClawError> {
//!         if tool_name == "Bash" {
//!             // Strip dangerous flags and return sanitized input
//!             let safe_input = json!({ "command": "echo 'sanitized'" });
//!             return Ok(PermissionDecision::Allow { updated_input: Some(safe_input) });
//!         }
//!         Ok(PermissionDecision::Allow { updated_input: None })
//!     }
//! }
//! ```

mod handler;

pub use handler::{DefaultPermissionHandler, DefaultPermissionHandlerBuilder};

/// Rich result type for permission decisions.
///
/// Replaces the previous `bool` return from `CanUseToolHandler::can_use_tool()`.
/// Provides additional capabilities:
///
/// - [`PermissionDecision::Allow`]: Allow tool execution, optionally replacing
///   the tool input with a sanitized or modified version.
/// - [`PermissionDecision::Deny`]: Deny tool execution, optionally interrupting
///   the entire session (not just the current tool use).
///
/// # Examples
///
/// ```rust
/// use rusty_claw::permissions::PermissionDecision;
/// use serde_json::json;
///
/// // Simple allow
/// let decision = PermissionDecision::Allow { updated_input: None };
///
/// // Allow with sanitized input (strips dangerous args)
/// let safe_input = json!({ "command": "echo safe" });
/// let decision = PermissionDecision::Allow { updated_input: Some(safe_input) };
///
/// // Simple deny
/// let decision = PermissionDecision::Deny { interrupt: false };
///
/// // Deny and interrupt the session entirely
/// let decision = PermissionDecision::Deny { interrupt: true };
/// ```
#[derive(Debug, Clone)]
pub enum PermissionDecision {
    /// Allow tool execution.
    ///
    /// If `updated_input` is `Some`, the CLI will use the provided value as the
    /// tool input instead of the original. This allows permission handlers to
    /// sanitize or transform arguments (e.g., stripping dangerous flags from
    /// a bash command) while still permitting execution.
    Allow {
        /// Optional replacement for the tool's input JSON.
        ///
        /// When `Some`, the CLI uses this value instead of the original input.
        /// When `None`, the original input is used unchanged.
        updated_input: Option<serde_json::Value>,
    },

    /// Deny tool execution.
    ///
    /// When `interrupt` is `true`, the entire agent session is interrupted,
    /// not just the current tool use. This is appropriate when a dangerous
    /// operation is detected that should halt all further processing.
    Deny {
        /// Whether to interrupt the entire session.
        ///
        /// - `false`: Deny this tool use only; the agent may continue.
        /// - `true`: Interrupt the entire session immediately.
        interrupt: bool,
    },
}

impl PermissionDecision {
    /// Returns `true` if this decision allows the tool to execute.
    pub fn is_allowed(&self) -> bool {
        matches!(self, PermissionDecision::Allow { .. })
    }

    /// Returns `true` if this decision denies the tool.
    pub fn is_denied(&self) -> bool {
        matches!(self, PermissionDecision::Deny { .. })
    }

    /// Returns the updated input if this is an `Allow` decision with one.
    pub fn updated_input(&self) -> Option<&serde_json::Value> {
        match self {
            PermissionDecision::Allow { updated_input } => updated_input.as_ref(),
            PermissionDecision::Deny { .. } => None,
        }
    }
}

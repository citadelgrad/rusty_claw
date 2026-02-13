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

mod handler;

pub use handler::DefaultPermissionHandler;

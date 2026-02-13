//! Error types for the Rusty Claw SDK
//!
//! This module defines the error hierarchy for the rusty_claw crate using `thiserror`.
//! All SDK operations that can fail return `Result<T, ClawError>`.
//!
//! # Error Variants
//!
//! - [`ClawError::CliNotFound`]: Claude Code CLI binary not found during discovery
//! - [`ClawError::InvalidCliVersion`]: CLI version is older than required (< 2.0.0)
//! - [`ClawError::Connection`]: Transport connection failures
//! - [`ClawError::Process`]: CLI process crashes or non-zero exits
//! - [`ClawError::JsonDecode`]: JSONL parsing errors (auto-converts from `serde_json::Error`)
//! - [`ClawError::MessageParse`]: Malformed control protocol messages
//! - [`ClawError::ControlTimeout`]: Control protocol request timeouts
//! - [`ClawError::ControlError`]: Control protocol semantic errors
//! - [`ClawError::Io`]: Filesystem and I/O operations (auto-converts from `std::io::Error`)
//! - [`ClawError::ToolExecution`]: MCP tool handler failures
//!
//! # Example
//!
//! ```rust
//! use rusty_claw::error::ClawError;
//!
//! fn example() -> Result<(), ClawError> {
//!     // Auto-conversion from std::io::Error
//!     let _file = std::fs::read_to_string("/nonexistent")?;
//!
//!     // Manual construction
//!     return Err(ClawError::CliNotFound);
//! }
//! ```

use thiserror::Error;

/// The main error type for all rusty_claw operations
///
/// This enum covers all error conditions that can occur when using the SDK,
/// from CLI discovery through transport communication to tool execution.
///
/// Two variants support automatic conversion via the `?` operator:
/// - `JsonDecode` from `serde_json::Error`
/// - `Io` from `std::io::Error`
#[derive(Error, Debug)]
pub enum ClawError {
    /// Claude Code CLI binary was not found
    ///
    /// This error occurs during CLI discovery when the `claude-code` binary
    /// cannot be located in PATH or at the specified `cli_path`.
    ///
    /// # Resolution
    /// - Install Claude Code CLI: `npm install -g @anthropic-ai/claude-code`
    /// - Or specify the CLI path explicitly when creating the transport
    #[error("Claude Code CLI not found. Install it or set cli_path.")]
    CliNotFound,

    /// Claude Code CLI version is incompatible
    ///
    /// This error occurs when the installed CLI version is older than the
    /// minimum required version (2.0.0).
    ///
    /// # Resolution
    /// - Upgrade Claude Code CLI: `npm update -g @anthropic-ai/claude-code`
    /// - Or install the latest version: `npm install -g @anthropic-ai/claude-code@latest`
    #[error("Invalid Claude CLI version: expected >= 2.0.0, found {version}")]
    InvalidCliVersion {
        /// The actual version string found (e.g., "1.5.2")
        version: String,
    },

    /// Failed to establish connection to Claude Code CLI
    ///
    /// This error occurs when the transport cannot connect to the CLI process,
    /// typically due to the process failing to start or crashing during initialization.
    #[error("Failed to connect to Claude Code CLI: {0}")]
    Connection(String),

    /// CLI process exited with a non-zero exit code
    ///
    /// This error captures process crashes or abnormal termination with stderr output.
    #[error("CLI process exited with code {code}: {stderr}")]
    Process {
        /// The exit code returned by the CLI process
        code: i32,
        /// Standard error output from the failed process
        stderr: String,
    },

    /// Failed to parse JSON from CLI output
    ///
    /// This error is automatically converted from `serde_json::Error` when parsing
    /// JSONL messages from the CLI transport.
    #[error("Failed to parse JSON from CLI: {0}")]
    JsonDecode(#[from] serde_json::Error),

    /// Failed to parse a control protocol message
    ///
    /// This error occurs when a message has valid JSON structure but doesn't
    /// conform to the expected control protocol schema.
    #[error("Failed to parse message: {reason}")]
    MessageParse {
        /// Description of what went wrong during parsing
        reason: String,
        /// The raw message content that failed to parse
        raw: String,
    },

    /// Control protocol request timed out
    ///
    /// This error occurs when a control protocol request doesn't receive a
    /// response within the configured timeout period.
    #[error("Control protocol timeout waiting for {subtype}")]
    ControlTimeout {
        /// The message subtype that timed out (e.g., "prompt_response")
        subtype: String,
    },

    /// Control protocol semantic error
    ///
    /// This error occurs when the control protocol returns an error response,
    /// such as permission denial or invalid request parameters.
    #[error("Control protocol error: {0}")]
    ControlError(String),

    /// I/O operation failed
    ///
    /// This error is automatically converted from `std::io::Error` for filesystem
    /// operations, process spawning, and stdio communication.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// MCP tool execution failed
    ///
    /// This error occurs when a registered tool handler returns an error during execution.
    #[error("Tool execution failed: {0}")]
    ToolExecution(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_not_found_message() {
        let err = ClawError::CliNotFound;
        assert_eq!(
            err.to_string(),
            "Claude Code CLI not found. Install it or set cli_path."
        );
    }

    #[test]
    fn test_invalid_cli_version_message() {
        let err = ClawError::InvalidCliVersion {
            version: "1.5.2".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Invalid Claude CLI version: expected >= 2.0.0, found 1.5.2"
        );
    }

    #[test]
    fn test_connection_error_message() {
        let err = ClawError::Connection("timeout".to_string());
        assert_eq!(
            err.to_string(),
            "Failed to connect to Claude Code CLI: timeout"
        );
    }

    #[test]
    fn test_process_error_message() {
        let err = ClawError::Process {
            code: 1,
            stderr: "permission denied".to_string(),
        };
        assert!(err.to_string().contains("code 1"));
        assert!(err.to_string().contains("permission denied"));
    }

    #[test]
    fn test_message_parse_error() {
        let err = ClawError::MessageParse {
            reason: "missing required field".to_string(),
            raw: r#"{"incomplete": true}"#.to_string(),
        };
        assert!(err.to_string().contains("missing required field"));
    }

    #[test]
    fn test_control_timeout_error() {
        let err = ClawError::ControlTimeout {
            subtype: "prompt_response".to_string(),
        };
        assert!(err.to_string().contains("prompt_response"));
    }

    #[test]
    fn test_control_error() {
        let err = ClawError::ControlError("permission denied".to_string());
        assert!(err.to_string().contains("permission denied"));
    }

    #[test]
    fn test_tool_execution_error() {
        let err = ClawError::ToolExecution("handler panicked".to_string());
        assert!(err.to_string().contains("handler panicked"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let claw_err: ClawError = io_err.into();
        assert!(claw_err.to_string().contains("file not found"));
    }

    #[test]
    fn test_json_error_conversion() {
        let json_str = "{ invalid json }";
        let json_err = serde_json::from_str::<serde_json::Value>(json_str).unwrap_err();
        let claw_err: ClawError = json_err.into();
        assert!(claw_err.to_string().contains("parse"));
    }

    #[test]
    fn test_result_with_question_mark_io() {
        fn read_file() -> Result<String, ClawError> {
            // This should auto-convert std::io::Error to ClawError::Io
            Ok(std::fs::read_to_string("/nonexistent/file.txt")?)
        }

        let result = read_file();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ClawError::Io(_)));
    }

    #[test]
    fn test_result_with_question_mark_json() {
        fn parse_json() -> Result<serde_json::Value, ClawError> {
            // This should auto-convert serde_json::Error to ClawError::JsonDecode
            Ok(serde_json::from_str("{ invalid }")?)
        }

        let result = parse_json();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ClawError::JsonDecode(_)));
    }
}

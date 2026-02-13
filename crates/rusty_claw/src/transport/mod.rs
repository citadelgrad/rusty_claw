//! Transport abstraction for communicating with Claude Code CLI
//!
//! This module provides the `Transport` trait, which defines an async interface for
//! establishing connections, sending messages, and receiving responses from the Claude CLI.
//!
//! # Overview
//!
//! The transport layer handles:
//! - CLI discovery and version validation
//! - Process lifecycle management (spawning, monitoring, shutdown)
//! - Bidirectional communication (stdin writes, stdout reads)
//! - NDJSON message framing and parsing
//! - Error handling and recovery
//!
//! # Default Implementation
//!
//! `SubprocessCLITransport` is the default implementation that spawns the `claude` CLI
//! as a subprocess and communicates over stdin/stdout pipes.
//!
//! # Custom Transports
//!
//! The `Transport` trait can be implemented for custom transports (e.g., remote connections,
//! mock transports for testing, or alternative CLI implementations).
//!
//! # Example
//!
//! ```ignore
//! use rusty_claw::transport::{Transport, SubprocessCLITransport};
//!
//! // Create transport with automatic CLI discovery
//! let mut transport = SubprocessCLITransport::new(
//!     None, // Will auto-discover 'claude' in PATH
//!     vec!["--output-format=stream-json".to_string()]
//! );
//!
//! // Establish connection (discovers and validates CLI version)
//! transport.connect().await?;
//!
//! // Get message receiver
//! let mut messages = transport.messages();
//!
//! // Send a message
//! transport.write(b"{\"type\":\"user\",\"message\":\"Hello\"}\n").await?;
//!
//! // Receive messages
//! while let Some(result) = messages.recv().await {
//!     match result {
//!         Ok(msg) => println!("Received: {:?}", msg),
//!         Err(e) => eprintln!("Error: {}", e),
//!     }
//! }
//!
//! // Clean shutdown
//! transport.close().await?;
//! ```

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::error::ClawError;

mod subprocess;
mod discovery;

pub use subprocess::SubprocessCLITransport;
pub use discovery::CliDiscovery;

/// Abstract transport for communicating with Claude Code CLI.
///
/// The default implementation ([`SubprocessCLITransport`]) spawns a subprocess,
/// but this trait enables custom transports (remote connections, mock for testing).
///
/// # Lifecycle
///
/// 1. **Create** - Construct transport with configuration
/// 2. **Connect** - Call [`connect()`](Transport::connect) to establish connection
/// 3. **Communicate** - Use [`write()`](Transport::write) and [`messages()`](Transport::messages)
/// 4. **Shutdown** - Call [`close()`](Transport::close) for graceful cleanup
///
/// # Thread Safety
///
/// All methods can be called from multiple tasks concurrently (`Send + Sync`).
/// The transport implementation handles internal synchronization.
///
/// # Message Channel
///
/// [`messages()`](Transport::messages) returns an [`UnboundedReceiver`](mpsc::UnboundedReceiver)
/// for incoming NDJSON messages. This method can only be called **once** per connection -
/// subsequent calls will panic. Store the receiver and pass it to consumers.
#[async_trait]
pub trait Transport: Send + Sync {
    /// Establish the connection (spawn process, open socket, etc.)
    ///
    /// This method must be called before any other operations. It may spawn
    /// background tasks for reading stdout and monitoring process health.
    ///
    /// # Errors
    ///
    /// - [`ClawError::CliNotFound`] if the CLI executable is not found
    /// - [`ClawError::Connection`] if the connection cannot be established
    /// - [`ClawError::Io`] for I/O errors during spawning
    async fn connect(&mut self) -> Result<(), ClawError>;

    /// Write a JSON message to the CLI's stdin
    ///
    /// The message should be a complete JSON object followed by a newline (NDJSON format).
    /// This method is safe to call concurrently from multiple tasks.
    ///
    /// # Arguments
    ///
    /// * `message` - Raw JSON bytes (must include trailing newline)
    ///
    /// # Errors
    ///
    /// - [`ClawError::Io`] if the write fails (e.g., stdin closed)
    /// - [`ClawError::Connection`] if not connected
    async fn write(&self, message: &[u8]) -> Result<(), ClawError>;

    /// Returns a receiver for incoming NDJSON messages from stdout.
    ///
    /// Messages are parsed by the transport into raw [`serde_json::Value`].
    /// The caller is responsible for further parsing into typed [`Message`](crate::messages::Message) structs.
    ///
    /// # Important
    ///
    /// This method can only be called **once** per connection. The receiver is moved
    /// out of the transport, and subsequent calls will panic. Store the receiver
    /// in your application and pass it to message consumers.
    ///
    /// # Returns
    ///
    /// An [`UnboundedReceiver`](mpsc::UnboundedReceiver) that yields:
    /// - `Ok(Value)` for successfully parsed NDJSON messages
    /// - `Err(ClawError::JsonDecode)` for malformed JSON
    /// - Channel closes when the process exits or stdout is closed
    ///
    /// # Panics
    ///
    /// Panics if called more than once per connection.
    fn messages(&self) -> mpsc::UnboundedReceiver<Result<Value, ClawError>>;

    /// Signal end of input (close stdin)
    ///
    /// This tells the CLI that no more messages will be sent. The CLI may continue
    /// to send response messages on stdout before exiting.
    ///
    /// # Errors
    ///
    /// - [`ClawError::Io`] if stdin cannot be closed
    async fn end_input(&self) -> Result<(), ClawError>;

    /// Close the transport and clean up resources
    ///
    /// For subprocess transports, this:
    /// 1. Closes stdin (if not already closed)
    /// 2. Waits for graceful exit
    /// 3. Sends SIGTERM if process doesn't exit
    /// 4. Sends SIGKILL after timeout
    ///
    /// This method is idempotent - calling it multiple times is safe.
    ///
    /// # Errors
    ///
    /// - [`ClawError::Process`] if the process exits with non-zero status
    async fn close(&mut self) -> Result<(), ClawError>;

    /// Whether the transport is connected and ready
    ///
    /// Returns `true` if:
    /// - [`connect()`](Transport::connect) has been called successfully
    /// - The underlying process/connection is still alive
    /// - Messages can be sent and received
    fn is_ready(&self) -> bool;
}

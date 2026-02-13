//! ClaudeClient for interactive sessions with Claude CLI
//!
//! The `ClaudeClient` provides a high-level API for maintaining long-running interactive sessions
//! with the Claude Code CLI. Unlike the one-shot [`query()`](crate::query) API, `ClaudeClient`
//! maintains a persistent connection and allows:
//!
//! - **Multiple message exchanges** - Send messages and receive streaming responses
//! - **Session control** - Interrupt execution, change models, modify permission modes
//! - **Handler registration** - Install callbacks for tool permission checks, hooks, and MCP
//! - **Full control protocol access** - All control operations supported by the CLI
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────┐
//! │                     ClaudeClient                         │
//! │                                                          │
//! │  Session Management          Control Operations         │
//! │  • connect()                 • interrupt()              │
//! │  • send_message()            • set_permission_mode()    │
//! │  • close()                   • set_model()              │
//! │                              • mcp_status()             │
//! │                              • rewind_files()           │
//! │                                                          │
//! │  ┌────────────────────────────────────────────────────┐ │
//! │  │        ControlProtocol (request/response)         │ │
//! │  └────────────────────────────────────────────────────┘ │
//! │                          ↕                               │
//! │  ┌────────────────────────────────────────────────────┐ │
//! │  │        Transport (SubprocessCLITransport)         │ │
//! │  └────────────────────────────────────────────────────┘ │
//! └──────────────────────────────────────────────────────────┘
//!           ↓ ResponseStream                    ↑
//!    Assistant/Result/System           send_message()
//! ```
//!
//! # Example: Basic Session
//!
//! ```no_run
//! use rusty_claw::prelude::*;
//! use tokio_stream::StreamExt;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create and connect client
//!     let options = ClaudeAgentOptions::builder()
//!         .max_turns(10)
//!         .permission_mode(PermissionMode::AcceptEdits)
//!         .build();
//!
//!     let mut client = ClaudeClient::new(options)?;
//!     client.connect().await?;
//!
//!     // Send a message and stream responses
//!     let mut stream = client.send_message("What files are in this directory?").await?;
//!
//!     while let Some(result) = stream.next().await {
//!         match result {
//!             Ok(Message::Assistant(msg)) => {
//!                 for block in msg.message.content {
//!                     if let ContentBlock::Text { text } = block {
//!                         println!("Claude: {}", text);
//!                     }
//!                 }
//!             }
//!             Ok(Message::Result(msg)) => {
//!                 println!("Result: {:?}", msg);
//!                 break;
//!             }
//!             Ok(_) => {}
//!             Err(e) => eprintln!("Error: {}", e),
//!         }
//!     }
//!
//!     client.close().await?;
//!     Ok(())
//! }
//! ```
//!
//! # Example: Control Operations
//!
//! ```no_run
//! use rusty_claw::prelude::*;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let options = ClaudeAgentOptions::default();
//! # let mut client = ClaudeClient::new(options)?;
//! # client.connect().await?;
//! // Start a task
//! let mut stream = client.send_message("Write a long essay about Rust").await?;
//!
//! // Change your mind and interrupt
//! client.interrupt().await?;
//!
//! // Switch to a faster model
//! client.set_model("claude-sonnet-4-5").await?;
//!
//! // Change permission mode
//! client.set_permission_mode(PermissionMode::Ask).await?;
//! # Ok(())
//! # }
//! ```

use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio_stream::Stream;
use serde_json::Value;

use crate::control::ControlProtocol;
use crate::control::handlers::{HookHandler, McpMessageHandler, CanUseToolHandler};
use crate::error::ClawError;
use crate::messages::Message;
use crate::options::{ClaudeAgentOptions, PermissionMode};
use crate::transport::Transport;

/// Client for interactive sessions with Claude CLI
///
/// `ClaudeClient` maintains a persistent connection to the Claude Code CLI subprocess
/// and provides methods for sending messages, receiving streaming responses, and
/// controlling the session (interrupt, model changes, permission modes).
///
/// # Thread Safety
///
/// `ClaudeClient` is `Send + Sync` but message receiving is single-consumer.
/// After calling `send_message()`, the returned `ResponseStream` owns the message
/// receiver and is the only way to receive messages from that point forward.
///
/// # Lifecycle
///
/// 1. **Create** - `new()` with configuration options
/// 2. **Connect** - `connect()` spawns CLI subprocess and initializes session
/// 3. **Interact** - `send_message()` and consume `ResponseStream`
/// 4. **Close** - `close()` gracefully shuts down the CLI subprocess
///
/// # Example
///
/// ```no_run
/// use rusty_claw::prelude::*;
/// use tokio_stream::StreamExt;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let options = ClaudeAgentOptions::default();
/// let mut client = ClaudeClient::new(options)?;
/// client.connect().await?;
///
/// let mut stream = client.send_message("Hello!").await?;
/// while let Some(msg) = stream.next().await {
///     println!("{:?}", msg);
/// }
///
/// client.close().await?;
/// # Ok(())
/// # }
/// ```
pub struct ClaudeClient {
    /// Control protocol for request/response handling
    control: Option<Arc<ControlProtocol>>,

    /// Transport layer (stored as Option to allow taking ownership in connect)
    transport: Option<Arc<dyn Transport>>,

    /// Session configuration
    options: ClaudeAgentOptions,

    /// Message receiver from transport (taken on send_message)
    #[allow(clippy::type_complexity)]
    message_rx: Arc<Mutex<Option<mpsc::UnboundedReceiver<Result<Value, ClawError>>>>>,

    /// Session initialization state
    is_initialized: Arc<Mutex<bool>>,
}

impl ClaudeClient {
    /// Create a new client with the given options
    ///
    /// This does not connect to the CLI yet. Call [`connect()`](Self::connect) to
    /// establish the connection and initialize the session.
    ///
    /// # Arguments
    ///
    /// * `options` - Configuration for the Claude session
    ///
    /// # Example
    ///
    /// ```
    /// use rusty_claw::prelude::*;
    ///
    /// let options = ClaudeAgentOptions::builder()
    ///     .max_turns(5)
    ///     .permission_mode(PermissionMode::AcceptEdits)
    ///     .build();
    /// let client = ClaudeClient::new(options).unwrap();
    /// ```
    pub fn new(options: ClaudeAgentOptions) -> Result<Self, ClawError> {
        Ok(Self {
            control: None,
            transport: None,
            options,
            message_rx: Arc::new(Mutex::new(None)),
            is_initialized: Arc::new(Mutex::new(false)),
        })
    }

    /// Check if the client is connected and ready
    ///
    /// Returns `true` if the transport is connected and the session is initialized.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rusty_claw::prelude::*;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let options = ClaudeAgentOptions::default();
    /// let mut client = ClaudeClient::new(options)?;
    /// assert!(!client.is_connected());
    ///
    /// client.connect().await?;
    /// assert!(client.is_connected());
    /// # Ok(())
    /// # }
    /// ```
    pub fn is_connected(&self) -> bool {
        self.transport
            .as_ref()
            .map(|t| t.is_ready())
            .unwrap_or(false)
            && *self.is_initialized.blocking_lock()
    }

    /// Connect to the Claude CLI and initialize the session
    ///
    /// This method:
    /// 1. Creates a SubprocessCLITransport with CLI auto-discovery
    /// 2. Connects to the CLI subprocess
    /// 3. Creates a ControlProtocol instance
    /// 4. Initializes the session with the configured options
    /// 5. Stores the message receiver for later use
    ///
    /// # Errors
    ///
    /// - `ClawError::CliNotFound` - Claude CLI binary not found
    /// - `ClawError::InvalidCliVersion` - CLI version too old (< 2.0.0)
    /// - `ClawError::Connection` - Failed to connect to CLI
    /// - `ClawError::ControlTimeout` - Initialization request timed out
    /// - `ClawError::ControlError` - Initialization failed
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rusty_claw::prelude::*;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let options = ClaudeAgentOptions::default();
    /// let mut client = ClaudeClient::new(options)?;
    /// client.connect().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect(&mut self) -> Result<(), ClawError> {
        use crate::transport::SubprocessCLITransport;

        // Create CLI args for interactive mode (no prompt yet)
        // We'll send messages via stdin after connection
        let mut cli_args = vec![
            "--output-format=stream-json".to_string(),
            "--verbose".to_string(),
        ];

        // Add options (but not the prompt - that comes via send_message)
        if let Some(max_turns) = self.options.max_turns {
            cli_args.push(format!("--max-turns={}", max_turns));
        }
        if let Some(model) = &self.options.model {
            cli_args.push(format!("--model={}", model));
        }
        if let Some(mode) = &self.options.permission_mode {
            cli_args.push(format!("--permission-mode={}", mode.to_cli_arg()));
        }
        // Add other options as needed...

        // Create and connect transport
        let mut transport = SubprocessCLITransport::new(None, cli_args);
        transport.connect().await?;

        // Get message receiver before wrapping in Arc
        let message_rx = transport.messages();

        // Wrap transport in Arc for sharing
        let transport_arc: Arc<dyn Transport> = Arc::new(transport);

        // Create control protocol
        let control = Arc::new(ControlProtocol::new(transport_arc.clone()));

        // Initialize session
        control.initialize(&self.options).await?;

        // Store state
        self.transport = Some(transport_arc);
        self.control = Some(control);
        *self.message_rx.lock().await = Some(message_rx);
        *self.is_initialized.lock().await = true;

        Ok(())
    }

    /// Close the session gracefully
    ///
    /// This method:
    /// 1. Ends input to the CLI (signals no more messages)
    /// 2. Waits for the CLI subprocess to exit
    /// 3. Cleans up internal state
    ///
    /// After calling `close()`, the client cannot be used again. Create a new
    /// client if you need to start another session.
    ///
    /// # Errors
    ///
    /// - `ClawError::Connection` - Failed to close transport cleanly
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rusty_claw::prelude::*;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let options = ClaudeAgentOptions::default();
    /// # let mut client = ClaudeClient::new(options)?;
    /// # client.connect().await?;
    /// client.close().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn close(&mut self) -> Result<(), ClawError> {
        if let Some(transport) = &self.transport {
            // End input stream
            transport.end_input().await?;

            // Note: Can't call close() on Arc<dyn Transport> because it requires &mut self
            // The transport will be cleaned up when dropped
        }

        // Clear state
        *self.is_initialized.lock().await = false;
        self.transport = None;
        self.control = None;

        Ok(())
    }

    // Message sending methods

    /// Send a message to Claude and get a stream of responses
    ///
    /// This method:
    /// 1. Writes a user message to the CLI stdin
    /// 2. Takes the message receiver (single-use)
    /// 3. Returns a `ResponseStream` that yields responses
    ///
    /// **Note:** `send_message()` can only be called once per client instance because
    /// it takes ownership of the message receiver. After the stream completes, you
    /// must create a new client for additional interactions.
    ///
    /// # Arguments
    ///
    /// * `content` - The message text to send to Claude
    ///
    /// # Returns
    ///
    /// A `ResponseStream` that yields `Message` items until the CLI closes the stream.
    ///
    /// # Errors
    ///
    /// - `ClawError::Connection` - Not connected (call `connect()` first)
    /// - `ClawError::Connection` - Message receiver already taken
    /// - `ClawError::Io` - Failed to write message to CLI
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rusty_claw::prelude::*;
    /// # use tokio_stream::StreamExt;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let options = ClaudeAgentOptions::default();
    /// # let mut client = ClaudeClient::new(options)?;
    /// # client.connect().await?;
    /// let mut stream = client.send_message("What is 2+2?").await?;
    ///
    /// while let Some(result) = stream.next().await {
    ///     match result {
    ///         Ok(Message::Assistant(msg)) => println!("Claude: {:?}", msg),
    ///         Ok(Message::Result(_)) => break,
    ///         Ok(_) => {},
    ///         Err(e) => eprintln!("Error: {}", e),
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send_message(
        &self,
        content: impl Into<String>,
    ) -> Result<ResponseStream, ClawError> {
        // Check if connected
        if self.control.is_none() {
            return Err(ClawError::Connection(
                "Not connected. Call connect() first.".to_string(),
            ));
        }

        // Write the message
        self.write_message(content.into().as_str()).await?;

        // Take the message receiver
        let mut rx_lock = self.message_rx.lock().await;
        let rx = rx_lock.take().ok_or_else(|| {
            ClawError::Connection("Message receiver already taken. send_message() can only be called once per client.".to_string())
        })?;

        // Create and return response stream
        Ok(ResponseStream::new(rx, self.control.as_ref().unwrap().clone()))
    }

    /// Write a user message to the CLI stdin
    ///
    /// This is an internal helper that formats and sends a user message.
    ///
    /// # Message Format
    ///
    /// ```json
    /// {
    ///   "type": "user",
    ///   "message": {
    ///     "role": "user",
    ///     "content": [{"type": "text", "text": "..."}]
    ///   }
    /// }
    /// ```
    async fn write_message(&self, content: &str) -> Result<(), ClawError> {
        use serde_json::json;

        let transport = self.transport.as_ref().ok_or_else(|| {
            ClawError::Connection("Transport not available".to_string())
        })?;

        // Format user message
        let message = json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": content
                    }
                ]
            }
        });

        // Serialize to bytes
        let mut bytes = serde_json::to_vec(&message).map_err(|e| {
            ClawError::Connection(format!("Failed to serialize user message: {}", e))
        })?;
        bytes.push(b'\n'); // NDJSON requires newline

        // Write to transport
        transport.write(&bytes).await?;

        Ok(())
    }

    // Control operations

    /// Interrupt the current agent execution
    ///
    /// Sends a cancellation signal to stop ongoing processing. The CLI will finish
    /// the current operation and return control.
    ///
    /// # Errors
    ///
    /// - `ClawError::Connection` - Not connected
    /// - `ClawError::ControlTimeout` - Request timed out
    /// - `ClawError::ControlError` - Interrupt failed
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rusty_claw::prelude::*;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let options = ClaudeAgentOptions::default();
    /// # let mut client = ClaudeClient::new(options)?;
    /// # client.connect().await?;
    /// // Start a long-running task
    /// let _stream = client.send_message("Write a very long essay").await?;
    ///
    /// // Change your mind and interrupt
    /// client.interrupt().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn interrupt(&self) -> Result<(), ClawError> {
        use crate::control::messages::{ControlRequest, ControlResponse};

        let control = self.control.as_ref().ok_or_else(|| {
            ClawError::Connection("Not connected. Call connect() first.".to_string())
        })?;

        let response = control.request(ControlRequest::Interrupt).await?;

        match response {
            ControlResponse::Success { .. } => Ok(()),
            ControlResponse::Error { error, .. } => {
                Err(ClawError::ControlError(format!("Interrupt failed: {}", error)))
            }
        }
    }

    /// Change permission mode during the session
    ///
    /// Dynamically adjusts how tool permissions are handled. This allows you to
    /// switch between different permission modes without restarting the session.
    ///
    /// # Arguments
    ///
    /// * `mode` - New permission mode (e.g., Ask, Deny, Allow)
    ///
    /// # Errors
    ///
    /// - `ClawError::Connection` - Not connected
    /// - `ClawError::ControlTimeout` - Request timed out
    /// - `ClawError::ControlError` - Mode change failed
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rusty_claw::prelude::*;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let options = ClaudeAgentOptions::default();
    /// # let mut client = ClaudeClient::new(options)?;
    /// # client.connect().await?;
    /// // Switch to manual permission mode
    /// client.set_permission_mode(PermissionMode::Ask).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn set_permission_mode(&self, mode: PermissionMode) -> Result<(), ClawError> {
        use crate::control::messages::{ControlRequest, ControlResponse};

        let control = self.control.as_ref().ok_or_else(|| {
            ClawError::Connection("Not connected. Call connect() first.".to_string())
        })?;

        let response = control
            .request(ControlRequest::SetPermissionMode {
                mode: mode.to_cli_arg().to_string(),
            })
            .await?;

        match response {
            ControlResponse::Success { .. } => Ok(()),
            ControlResponse::Error { error, .. } => Err(ClawError::ControlError(format!(
                "Set permission mode failed: {}",
                error
            ))),
        }
    }

    /// Switch the active model during the session
    ///
    /// Changes which Claude model processes subsequent turns. Useful for switching
    /// between models based on task complexity or cost considerations.
    ///
    /// # Arguments
    ///
    /// * `model` - Model identifier (e.g., "claude-sonnet-4-5", "claude-opus-4-6")
    ///
    /// # Errors
    ///
    /// - `ClawError::Connection` - Not connected
    /// - `ClawError::ControlTimeout` - Request timed out
    /// - `ClawError::ControlError` - Model switch failed
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rusty_claw::prelude::*;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let options = ClaudeAgentOptions::default();
    /// # let mut client = ClaudeClient::new(options)?;
    /// # client.connect().await?;
    /// // Switch to a faster model
    /// client.set_model("claude-sonnet-4-5").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn set_model(&self, model: impl Into<String>) -> Result<(), ClawError> {
        use crate::control::messages::{ControlRequest, ControlResponse};

        let control = self.control.as_ref().ok_or_else(|| {
            ClawError::Connection("Not connected. Call connect() first.".to_string())
        })?;

        let response = control
            .request(ControlRequest::SetModel {
                model: model.into(),
            })
            .await?;

        match response {
            ControlResponse::Success { .. } => Ok(()),
            ControlResponse::Error { error, .. } => {
                Err(ClawError::ControlError(format!("Set model failed: {}", error)))
            }
        }
    }

    /// Query MCP server connection status
    ///
    /// Returns information about connected MCP servers, their tools, and status.
    ///
    /// # Returns
    ///
    /// JSON object with MCP server information
    ///
    /// # Errors
    ///
    /// - `ClawError::Connection` - Not connected
    /// - `ClawError::ControlTimeout` - Request timed out
    /// - `ClawError::ControlError` - Status query failed
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rusty_claw::prelude::*;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let options = ClaudeAgentOptions::default();
    /// # let mut client = ClaudeClient::new(options)?;
    /// # client.connect().await?;
    /// let status = client.mcp_status().await?;
    /// println!("MCP Status: {}", status);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn mcp_status(&self) -> Result<serde_json::Value, ClawError> {
        use crate::control::messages::{ControlRequest, ControlResponse};

        let control = self.control.as_ref().ok_or_else(|| {
            ClawError::Connection("Not connected. Call connect() first.".to_string())
        })?;

        let response = control.request(ControlRequest::McpStatus).await?;

        match response {
            ControlResponse::Success { data } => Ok(data),
            ControlResponse::Error { error, .. } => {
                Err(ClawError::ControlError(format!("MCP status query failed: {}", error)))
            }
        }
    }

    /// Rewind file state to a specific message
    ///
    /// Rolls back filesystem changes to the state at the given message ID. This is
    /// useful for undoing file modifications made by the agent.
    ///
    /// # Arguments
    ///
    /// * `message_id` - Message ID to rewind to
    ///
    /// # Errors
    ///
    /// - `ClawError::Connection` - Not connected
    /// - `ClawError::ControlTimeout` - Request timed out
    /// - `ClawError::ControlError` - Rewind failed
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rusty_claw::prelude::*;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let options = ClaudeAgentOptions::default();
    /// # let mut client = ClaudeClient::new(options)?;
    /// # client.connect().await?;
    /// // Rewind to a previous state
    /// client.rewind_files("msg_123").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn rewind_files(&self, message_id: impl Into<String>) -> Result<(), ClawError> {
        use crate::control::messages::{ControlRequest, ControlResponse};

        let control = self.control.as_ref().ok_or_else(|| {
            ClawError::Connection("Not connected. Call connect() first.".to_string())
        })?;

        let response = control
            .request(ControlRequest::RewindFiles {
                message_id: message_id.into(),
            })
            .await?;

        match response {
            ControlResponse::Success { .. } => Ok(()),
            ControlResponse::Error { error, .. } => {
                Err(ClawError::ControlError(format!("Rewind files failed: {}", error)))
            }
        }
    }

    // Handler registration

    /// Register a handler for can_use_tool permission requests
    ///
    /// The handler will be invoked whenever the CLI asks for permission to use a tool.
    /// This allows custom permission logic beyond the built-in permission modes.
    ///
    /// # Arguments
    ///
    /// * `handler` - Handler implementing `CanUseToolHandler` trait
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rusty_claw::prelude::*;
    /// # use std::sync::Arc;
    /// # use async_trait::async_trait;
    /// #
    /// # struct MyHandler;
    /// # #[async_trait]
    /// # impl CanUseToolHandler for MyHandler {
    /// #     async fn can_use_tool(&self, tool_name: &str, tool_input: &serde_json::Value) -> Result<bool, rusty_claw::error::ClawError> {
    /// #         Ok(true)
    /// #     }
    /// # }
    /// #
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let options = ClaudeAgentOptions::default();
    /// # let mut client = ClaudeClient::new(options)?;
    /// # client.connect().await?;
    /// let handler = Arc::new(MyHandler);
    /// client.register_can_use_tool_handler(handler).await;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn register_can_use_tool_handler(&self, handler: Arc<dyn CanUseToolHandler>) {
        if let Some(control) = &self.control {
            let mut handlers = control.handlers().await;
            handlers.register_can_use_tool(handler);
        }
    }

    /// Register a hook callback handler
    ///
    /// Hooks allow you to intercept and respond to lifecycle events like tool use,
    /// message processing, and error handling.
    ///
    /// # Arguments
    ///
    /// * `hook_id` - Unique identifier for this hook
    /// * `handler` - Handler implementing `HookHandler` trait
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rusty_claw::prelude::*;
    /// # use std::sync::Arc;
    /// # use async_trait::async_trait;
    /// #
    /// # struct MyHook;
    /// # #[async_trait]
    /// # impl HookHandler for MyHook {
    /// #     async fn call(&self, _event: HookEvent, input: serde_json::Value) -> Result<serde_json::Value, rusty_claw::error::ClawError> {
    /// #         Ok(serde_json::json!({"status": "ok"}))
    /// #     }
    /// # }
    /// #
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let options = ClaudeAgentOptions::default();
    /// # let mut client = ClaudeClient::new(options)?;
    /// # client.connect().await?;
    /// let handler = Arc::new(MyHook);
    /// client.register_hook("my_hook".to_string(), handler).await;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn register_hook(&self, hook_id: String, handler: Arc<dyn HookHandler>) {
        if let Some(control) = &self.control {
            let mut handlers = control.handlers().await;
            handlers.register_hook(hook_id, handler);
        }
    }

    /// Register an MCP message handler
    ///
    /// Handles MCP (Model Context Protocol) messages from the CLI, allowing you to
    /// implement custom MCP server functionality.
    ///
    /// # Arguments
    ///
    /// * `handler` - Handler implementing `McpMessageHandler` trait
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use rusty_claw::prelude::*;
    /// # use std::sync::Arc;
    /// # use async_trait::async_trait;
    /// #
    /// # struct MyMcpHandler;
    /// # #[async_trait]
    /// # impl McpMessageHandler for MyMcpHandler {
    /// #     async fn handle(&self, _server_name: &str, message: serde_json::Value) -> Result<serde_json::Value, rusty_claw::error::ClawError> {
    /// #         Ok(serde_json::json!({"result": "ok"}))
    /// #     }
    /// # }
    /// #
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let options = ClaudeAgentOptions::default();
    /// # let mut client = ClaudeClient::new(options)?;
    /// # client.connect().await?;
    /// let handler = Arc::new(MyMcpHandler);
    /// client.register_mcp_message_handler(handler).await;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn register_mcp_message_handler(&self, handler: Arc<dyn McpMessageHandler>) {
        if let Some(control) = &self.control {
            let mut handlers = control.handlers().await;
            handlers.register_mcp_message(handler);
        }
    }
}

/// Stream of response messages from Claude CLI
///
/// `ResponseStream` wraps the message receiver from the transport and:
/// - Parses raw JSON values into typed `Message` structs
/// - Routes control protocol messages internally (transparent to user)
/// - Yields only user-facing messages (Assistant, Result, System)
/// - Automatically ends when the CLI closes the stream
///
/// # Control Message Routing
///
/// The stream automatically handles control protocol messages:
/// - `Message::ControlRequest` → Routed to registered handlers
/// - `Message::ControlResponse` → Matched to pending requests
/// - Other messages → Yielded to the caller
///
/// This means you never see control messages in the stream - they are handled
/// internally by the control protocol.
///
/// # Example
///
/// ```no_run
/// use rusty_claw::prelude::*;
/// use tokio_stream::StreamExt;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let options = ClaudeAgentOptions::default();
/// # let mut client = ClaudeClient::new(options)?;
/// # client.connect().await?;
/// let mut stream = client.send_message("Hello").await?;
///
/// while let Some(result) = stream.next().await {
///     match result {
///         Ok(Message::Assistant(msg)) => println!("Assistant: {:?}", msg),
///         Ok(Message::Result(msg)) => {
///             println!("Done: {:?}", msg);
///             break;
///         }
///         Ok(_) => {}
///         Err(e) => eprintln!("Error: {}", e),
///     }
/// }
/// # Ok(())
/// # }
/// ```
pub struct ResponseStream {
    /// Receiver for raw messages from transport
    rx: mpsc::UnboundedReceiver<Result<Value, ClawError>>,

    /// Control protocol for routing control messages
    control: Arc<ControlProtocol>,

    /// Whether the stream has completed
    is_complete: bool,
}

impl ResponseStream {
    /// Create a new response stream
    fn new(
        rx: mpsc::UnboundedReceiver<Result<Value, ClawError>>,
        control: Arc<ControlProtocol>,
    ) -> Self {
        Self {
            rx,
            control,
            is_complete: false,
        }
    }

    /// Check if the stream has completed
    ///
    /// Returns `true` after the CLI has closed the output stream.
    pub fn is_complete(&self) -> bool {
        self.is_complete
    }
}

impl Stream for ResponseStream {
    type Item = Result<Message, ClawError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        use crate::control::messages::{ControlResponse, IncomingControlRequest};

        // Stream already complete
        if self.is_complete {
            return Poll::Ready(None);
        }

        // Poll the receiver
        match Pin::new(&mut self.rx).poll_recv(cx) {
            Poll::Ready(Some(Ok(value))) => {
                // Check message type first
                let msg_type = value.get("type").and_then(|v| v.as_str());

                match msg_type {
                    Some("control_request") => {
                        // Incoming control request from CLI - parse as IncomingControlRequest
                        // and route to handler (transparent to user)
                        let request_id = value
                            .get("request_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();

                        // Extract the request field and parse as IncomingControlRequest
                        if let Some(request_val) = value.get("request") {
                            match serde_json::from_value::<IncomingControlRequest>(request_val.clone()) {
                                Ok(incoming) => {
                                    let control = self.control.clone();
                                    tokio::spawn(async move {
                                        control.handle_incoming(&request_id, incoming).await;
                                    });
                                    // Control messages are transparent - poll again
                                    cx.waker().wake_by_ref();
                                    Poll::Pending
                                }
                                Err(e) => Poll::Ready(Some(Err(ClawError::MessageParse {
                                    reason: format!("Failed to parse incoming control request: {}", e),
                                    raw: value.to_string(),
                                }))),
                            }
                        } else {
                            Poll::Ready(Some(Err(ClawError::MessageParse {
                                reason: "control_request missing 'request' field".to_string(),
                                raw: value.to_string(),
                            })))
                        }
                    }
                    Some("control_response") => {
                        // Control response - parse and route to pending request
                        let request_id = value
                            .get("request_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();

                        if let Some(response_val) = value.get("response") {
                            match serde_json::from_value::<ControlResponse>(response_val.clone()) {
                                Ok(response) => {
                                    let control = self.control.clone();
                                    tokio::spawn(async move {
                                        control.handle_response(&request_id, response).await;
                                    });
                                    // Control messages are transparent - poll again
                                    cx.waker().wake_by_ref();
                                    Poll::Pending
                                }
                                Err(e) => Poll::Ready(Some(Err(ClawError::MessageParse {
                                    reason: format!("Failed to parse control response: {}", e),
                                    raw: value.to_string(),
                                }))),
                            }
                        } else {
                            Poll::Ready(Some(Err(ClawError::MessageParse {
                                reason: "control_response missing 'response' field".to_string(),
                                raw: value.to_string(),
                            })))
                        }
                    }
                    _ => {
                        // User-facing message - parse normally and yield
                        match serde_json::from_value::<Message>(value.clone()) {
                            Ok(message) => Poll::Ready(Some(Ok(message))),
                            Err(e) => Poll::Ready(Some(Err(ClawError::MessageParse {
                                reason: format!("Failed to parse message: {}", e),
                                raw: value.to_string(),
                            }))),
                        }
                    }
                }
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => {
                // Stream ended
                self.is_complete = true;
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::SubprocessCLITransport;

    #[test]
    fn test_new_client() {
        let options = ClaudeAgentOptions::default();
        let client = ClaudeClient::new(options);
        assert!(client.is_ok());
    }

    #[test]
    fn test_not_connected_initially() {
        let options = ClaudeAgentOptions::default();
        let client = ClaudeClient::new(options).unwrap();
        assert!(!client.is_connected());
    }

    #[test]
    fn test_response_stream_not_complete_initially() {
        let (_tx, rx) = mpsc::unbounded_channel();
        let transport = Arc::new(SubprocessCLITransport::new(None, vec![]));
        let control = Arc::new(ControlProtocol::new(transport));
        let stream = ResponseStream::new(rx, control);
        assert!(!stream.is_complete());
    }

    #[tokio::test]
    async fn test_send_message_without_connect() {
        let options = ClaudeAgentOptions::default();
        let client = ClaudeClient::new(options).unwrap();
        let result = client.send_message("test").await;
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(matches!(e, ClawError::Connection(_)));
        }
    }

    #[tokio::test]
    async fn test_interrupt_without_connect() {
        let options = ClaudeAgentOptions::default();
        let client = ClaudeClient::new(options).unwrap();
        let result = client.interrupt().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ClawError::Connection(_)));
    }

    #[tokio::test]
    async fn test_set_permission_mode_without_connect() {
        let options = ClaudeAgentOptions::default();
        let client = ClaudeClient::new(options).unwrap();
        let result = client.set_permission_mode(PermissionMode::Ask).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ClawError::Connection(_)));
    }

    #[tokio::test]
    async fn test_set_model_without_connect() {
        let options = ClaudeAgentOptions::default();
        let client = ClaudeClient::new(options).unwrap();
        let result = client.set_model("claude-sonnet-4-5").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ClawError::Connection(_)));
    }

    #[tokio::test]
    async fn test_mcp_status_without_connect() {
        let options = ClaudeAgentOptions::default();
        let client = ClaudeClient::new(options).unwrap();
        let result = client.mcp_status().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ClawError::Connection(_)));
    }

    #[tokio::test]
    async fn test_rewind_files_without_connect() {
        let options = ClaudeAgentOptions::default();
        let client = ClaudeClient::new(options).unwrap();
        let result = client.rewind_files("msg_123").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ClawError::Connection(_)));
    }

    #[test]
    fn test_client_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ClaudeClient>();
    }

    #[test]
    fn test_client_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ClaudeClient>();
    }

    #[test]
    fn test_response_stream_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ResponseStream>();
    }

    #[test]
    fn test_response_stream_is_unpin() {
        fn assert_unpin<T: Unpin>() {}
        assert_unpin::<ResponseStream>();
    }

    // Test that ClaudeClient builder pattern works with various options
    #[test]
    fn test_client_with_custom_options() {
        let options = ClaudeAgentOptions::builder()
            .max_turns(10)
            .permission_mode(PermissionMode::AcceptEdits)
            .model("claude-sonnet-4-5".to_string())
            .build();

        let client = ClaudeClient::new(options);
        assert!(client.is_ok());
    }

    // Test that multiple clients can be created
    #[test]
    fn test_multiple_clients() {
        let options1 = ClaudeAgentOptions::default();
        let options2 = ClaudeAgentOptions::default();

        let client1 = ClaudeClient::new(options1).unwrap();
        let client2 = ClaudeClient::new(options2).unwrap();

        assert!(!client1.is_connected());
        assert!(!client2.is_connected());
    }

    // Test handler registration when not connected doesn't panic
    #[tokio::test]
    async fn test_register_handlers_without_connect() {
        use crate::control::handlers::{HookHandler, McpMessageHandler, CanUseToolHandler};
        use crate::options::HookEvent;
        use async_trait::async_trait;
        use serde_json::{json, Value};

        struct TestPermHandler;
        #[async_trait]
        impl CanUseToolHandler for TestPermHandler {
            async fn can_use_tool(
                &self,
                _tool_name: &str,
                _tool_input: &serde_json::Value,
            ) -> Result<bool, ClawError> {
                Ok(true)
            }
        }

        struct TestHookHandler;
        #[async_trait]
        impl HookHandler for TestHookHandler {
            async fn call(
                &self,
                _hook_event: HookEvent,
                hook_input: Value,
            ) -> Result<Value, ClawError> {
                Ok(json!({ "echo": hook_input }))
            }
        }

        struct TestMcpHandler;
        #[async_trait]
        impl McpMessageHandler for TestMcpHandler {
            async fn handle(
                &self,
                _server_name: &str,
                _message: Value,
            ) -> Result<Value, ClawError> {
                Ok(json!({"result": "ok"}))
            }
        }

        let options = ClaudeAgentOptions::default();
        let client = ClaudeClient::new(options).unwrap();

        // These should not panic even when not connected
        client.register_can_use_tool_handler(Arc::new(TestPermHandler)).await;
        client.register_hook("test".to_string(), Arc::new(TestHookHandler)).await;
        client.register_mcp_message_handler(Arc::new(TestMcpHandler)).await;
    }
}

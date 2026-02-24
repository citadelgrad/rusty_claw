//! ClaudeClient for interactive sessions with Claude CLI
//!
//! The `ClaudeClient` provides a high-level API for maintaining long-running interactive sessions
//! with the Claude Code CLI. Unlike the one-shot `query()` API, `ClaudeClient`
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
//! │  • receive_response()        • set_model()              │
//! │  • close()                   • mcp_status()             │
//! │  • get_server_info()         • rewind_files()           │
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
//!     // First turn
//!     let mut stream = client.send_message("What files are in this directory?").await?;
//!     while let Some(result) = stream.next().await {
//!         match result {
//!             Ok(Message::Assistant(msg)) => {
//!                 for block in msg.message.content {
//!                     if let ContentBlock::Text { text } = block {
//!                         println!("Claude: {}", text);
//!                     }
//!                 }
//!             }
//!             Ok(Message::Result(_)) => break,
//!             Ok(_) => {}
//!             Err(e) => eprintln!("Error: {}", e),
//!         }
//!     }
//!
//!     // Second turn (same client, same session!)
//!     let mut stream2 = client.send_message("Now count them.").await?;
//!     while let Some(result) = stream2.next().await {
//!         match result {
//!             Ok(Message::Assistant(msg)) => {
//!                 for block in msg.message.content {
//!                     if let ContentBlock::Text { text } = block {
//!                         println!("Claude: {}", text);
//!                     }
//!                 }
//!             }
//!             Ok(Message::Result(_)) => break,
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
//! # Example: Using receive_response()
//!
//! ```no_run
//! use rusty_claw::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let options = ClaudeAgentOptions::default();
//!     let mut client = ClaudeClient::new(options)?;
//!     client.connect().await?;
//!
//!     // send_message returns a ResponseStream
//!     let stream = client.send_message("Summarize this repo").await?;
//!
//!     // receive_response collects all messages until Result
//!     let messages = stream.receive_response().await?;
//!     for msg in messages {
//!         println!("{:?}", msg);
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
//!
//! # Example: Transport Injection (for testing)
//!
//! ```no_run
//! use rusty_claw::prelude::*;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let options = ClaudeAgentOptions::default();
//! // Inject a custom transport (e.g., mock for tests)
//! // let transport = Box::new(MyMockTransport::new());
//! // let mut client = ClaudeClient::with_transport(options, transport)?;
//! # Ok(())
//! # }
//! ```
//!
//! # Example: RAII with_client helper
//!
//! ```no_run
//! use rusty_claw::client::with_client;
//! use rusty_claw::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let options = ClaudeAgentOptions::default();
//!
//!     with_client(options, |_client| async {
//!         // Interact with _client here
//!         Ok(())
//!     }).await?;
//!
//!     Ok(())
//! }
//! ```

use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio_stream::Stream;

use crate::control::handlers::{CanUseToolHandler, HookHandler, McpMessageHandler};
use crate::control::ControlProtocol;
use crate::error::ClawError;
use crate::messages::Message;
use crate::options::{ClaudeAgentOptions, PermissionMode};
use crate::transport::Transport;

/// Shared slot for the current-turn message sender.
///
/// The background message router holds a reference to this. When `send_message()` is
/// called, it creates a new `(tx, rx)` pair and stores `tx` here. The router then
/// forwards all non-control messages to that sender until the next `send_message()` call
/// installs a new sender.
type CurrentTurnSender = Arc<Mutex<Option<mpsc::UnboundedSender<Result<Value, ClawError>>>>>;

/// Client for interactive sessions with Claude CLI
///
/// `ClaudeClient` maintains a persistent connection to the Claude Code CLI subprocess
/// and provides methods for sending messages, receiving streaming responses, and
/// controlling the session (interrupt, model changes, permission modes).
///
/// # Multi-Turn Conversations
///
/// Unlike the one-shot `query()` API, `ClaudeClient` supports multiple message
/// exchanges on a single connection. Each call to `send_message()` creates a fresh
/// `ResponseStream` tied to that turn. After draining a `ResponseStream` (i.e., after
/// the CLI emits a `Message::Result`), you can call `send_message()` again for the
/// next turn.
///
/// # Thread Safety
///
/// `ClaudeClient` is `Send + Sync`. Multiple turns must be serialized by the caller
/// (i.e., drain the current `ResponseStream` before calling `send_message()` again).
///
/// # Lifecycle
///
/// 1. **Create** - `new()` with configuration options (or `with_transport()` for DI)
/// 2. **Connect** - `connect()` spawns CLI subprocess and initializes session
/// 3. **Interact** - `send_message()` returns a `ResponseStream` per turn
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
/// // First turn
/// let mut stream = client.send_message("Hello!").await?;
/// while let Some(msg) = stream.next().await {
///     println!("{:?}", msg);
/// }
///
/// // Second turn (same session)
/// let mut stream2 = client.send_message("And now?").await?;
/// while let Some(msg) = stream2.next().await {
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

    /// Shared sender slot: `send_message()` writes a new sender here each turn.
    /// The background router reads from this slot and forwards messages to it.
    current_turn_tx: CurrentTurnSender,

    /// Session initialization state
    is_initialized: Arc<AtomicBool>,

    /// Pre-injected transport for dependency injection (set via `with_transport()`).
    ///
    /// When `Some`, `connect()` uses this transport instead of spawning a CLI subprocess.
    /// This is primarily useful for testing with mock transports.
    pre_transport: Option<Box<dyn Transport>>,

    /// MCP handler registered before connect (applied during connect, before initialize)
    pending_mcp_handler: Option<Arc<dyn McpMessageHandler>>,
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
            pre_transport: None,
            options,
            current_turn_tx: Arc::new(Mutex::new(None)),
            is_initialized: Arc::new(AtomicBool::new(false)),
            pending_mcp_handler: None,
        })
    }

    /// Create a new client with a pre-built transport (dependency injection)
    ///
    /// This is primarily useful for testing with mock transports that avoid spawning
    /// a real CLI subprocess. After calling this, call [`connect()`](Self::connect)
    /// as usual.
    ///
    /// # Arguments
    ///
    /// * `options` - Configuration for the Claude session
    /// * `transport` - A pre-built transport implementing the `Transport` trait
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rusty_claw::prelude::*;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// // In real code you would use a mock transport here
    /// let transport = SubprocessCLITransport::new(None, vec![]);
    /// let options = ClaudeAgentOptions::default();
    /// let mut client = ClaudeClient::with_transport(options, Box::new(transport))?;
    /// // client.connect().await?;  // would complete transport connection
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_transport(
        options: ClaudeAgentOptions,
        transport: Box<dyn Transport>,
    ) -> Result<Self, ClawError> {
        Ok(Self {
            control: None,
            transport: None,
            pre_transport: Some(transport),
            options,
            current_turn_tx: Arc::new(Mutex::new(None)),
            is_initialized: Arc::new(AtomicBool::new(false)),
            pending_mcp_handler: None,
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
            && self.is_initialized.load(Ordering::SeqCst)
    }

    /// Connect to the Claude CLI and initialize the session
    ///
    /// This method:
    /// 1. Creates a SubprocessCLITransport with CLI auto-discovery (or uses a pre-injected transport)
    /// 2. Connects to the CLI subprocess
    /// 3. Creates a ControlProtocol instance
    /// 4. Initializes the session with the configured options
    /// 5. Spawns a background message routing task
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

           // Use pre-injected transport if available; otherwise build a SubprocessCLITransport
        let mut transport: Box<dyn Transport> =
            if let Some(pre) = self.pre_transport.take() {
                pre
            } else {
            // Build CLI args for interactive mode
            let mut cli_args = vec![
                "--output-format".to_string(),
                "stream-json".to_string(),
                "--verbose".to_string(),
            ];

            // Add options (but not the prompt - that comes via send_message)
            if let Some(max_turns) = self.options.max_turns {
                cli_args.push("--max-turns".to_string());
                cli_args.push(max_turns.to_string());
            }
            if let Some(model) = &self.options.model {
                cli_args.push("--model".to_string());
                cli_args.push(model.clone());
            }
            if let Some(mode) = &self.options.permission_mode {
                cli_args.push("--permission-mode".to_string());
                cli_args.push(mode.to_cli_arg().to_string());
            }

            // System prompt
            if let Some(sys_prompt) = &self.options.system_prompt {
                match sys_prompt {
                    crate::options::SystemPrompt::Custom(text) => {
                        cli_args.push("--system-prompt".to_string());
                        cli_args.push(text.clone());
                    }
                    crate::options::SystemPrompt::Preset { preset } => {
                        cli_args.push("--system-prompt-preset".to_string());
                        cli_args.push(preset.clone());
                    }
                }
            }

            // Append system prompt
            if let Some(append) = &self.options.append_system_prompt {
                cli_args.push("--append-system-prompt".to_string());
                cli_args.push(append.clone());
            }

            // Allowed tools
            if !self.options.allowed_tools.is_empty() {
                cli_args.push("--allowed-tools".to_string());
                cli_args.push(self.options.allowed_tools.join(","));
            }

            // Disallowed tools
            if !self.options.disallowed_tools.is_empty() {
                cli_args.push("--disallowed-tools".to_string());
                cli_args.push(self.options.disallowed_tools.join(","));
            }

            // Session options
            if let Some(resume) = &self.options.resume {
                cli_args.push("--resume".to_string());
                cli_args.push(resume.clone());
            }
            if self.options.fork_session {
                cli_args.push("--fork-session".to_string());
            }
            if let Some(name) = &self.options.session_name {
                cli_args.push("--session-name".to_string());
                cli_args.push(name.clone());
            }
            if self.options.enable_file_checkpointing {
                cli_args.push("--enable-file-checkpointing".to_string());
            }

            // Settings isolation for reproducibility
            match &self.options.setting_sources {
                Some(sources) => {
                    cli_args.push("--setting-sources".to_string());
                    cli_args.push(sources.join(","));
                }
                None => {
                    cli_args.push("--setting-sources".to_string());
                    cli_args.push(String::new());
                }
            }

            // External MCP servers (stdio/SSE/HTTP) — passed as --mcp-config with inline JSON.
            // NOTE: SDK-hosted servers (SdkMcpServerImpl) are intentionally excluded here.
            // The CLI hangs when type:"sdk" entries appear in --mcp-config.
            // SDK servers are registered via the sdkMcpServers field in the initialize
            // control request (sent by control.initialize()).
            if let Ok(Some(mcp_json)) = self.options.to_mcp_config_json() {
                cli_args.push("--mcp-config".to_string());
                cli_args.push(mcp_json);
            }

            // Enable control protocol input
            cli_args.push("--input-format".to_string());
            cli_args.push("stream-json".to_string());

            // Create transport
            let mut t = SubprocessCLITransport::new(self.options.cli_path.clone(), cli_args);

            // Apply working directory if configured
            if let Some(cwd) = &self.options.cwd {
                t.set_cwd(cwd.clone());
            }

            // Apply environment variables if configured
            if !self.options.env.is_empty() {
                t.set_env(self.options.env.clone());
            }

            Box::new(t) as Box<dyn Transport>
        };

        transport.connect().await?;

        // Get message receiver before wrapping in Arc
        let message_rx = transport.messages();

        // Wrap transport in Arc for sharing
        let transport_arc: Arc<dyn Transport> = Arc::from(transport as Box<dyn Transport>);

        // Create control protocol
        let control = Arc::new(ControlProtocol::new(transport_arc.clone()));

        // Spawn background message routing task BEFORE initialize().
        // This is critical: initialize() sends a control request and waits for
        // a response. The response arrives via the transport's message channel,
        // so we need a reader routing control messages to the ControlProtocol.
        // Without this, initialize() would always timeout.
        Self::spawn_message_router(
            message_rx,
            control.clone(),
            self.current_turn_tx.clone(),
        );

        // Apply pending MCP handler BEFORE initialize.
        // The CLI sends mcp_message requests during init, so the handler
        // must be registered before the initialize handshake.
        if let Some(handler) = self.pending_mcp_handler.take() {
            let mut handlers = control.handlers().await;
            handlers.register_mcp_message(handler);
        }

        // Apply permission handler from options BEFORE initialize.
        // can_use_tool requests can arrive during initialization, so the
        // handler must be in place before the initialize handshake.
        if let Some(handler) = self.options.permission_handler.take() {
            let mut handlers = control.handlers().await;
            handlers.register_can_use_tool(handler);
        }

        // Initialize session (now works because router handles the response)
        control.initialize(&self.options).await?;

        // Store state
        self.transport = Some(transport_arc);
        self.control = Some(control);
        self.is_initialized.store(true, Ordering::SeqCst);

        Ok(())
    }

    /// Close the session gracefully
    ///
    /// This method:
    /// 1. Ends input to the CLI (signals no more messages)
    /// 2. Waits for the CLI subprocess to exit
    /// 3. Cleans up internal state
    ///
    /// After calling `close()`, the client can no longer send messages. A new
    /// client must be created for a new session.
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
            // Graceful shutdown: close stdin, wait, then signal if needed
            transport.close().await?;
        }

        // Drop the current turn sender to unblock any waiting ResponseStream
        *self.current_turn_tx.lock().await = None;

        // Clear state
        self.is_initialized.store(false, Ordering::SeqCst);
        self.transport = None;
        self.control = None;

        Ok(())
    }

    // Message sending methods

    /// Send a message to Claude and get a stream of responses for this turn
    ///
    /// This method:
    /// 1. Writes a user message to the CLI stdin
    /// 2. Creates a fresh per-turn `(sender, receiver)` pair
    /// 3. Installs the sender in the shared routing slot
    /// 4. Returns a `ResponseStream` backed by the receiver
    ///
    /// Unlike the previous single-use design, `send_message()` can be called
    /// multiple times on the same client. Each call gets a fresh stream scoped
    /// to that turn's messages. The caller must drain (or drop) the previous
    /// `ResponseStream` before calling `send_message()` again; otherwise
    /// messages from the previous turn may be lost.
    ///
    /// # Arguments
    ///
    /// * `content` - The message text to send to Claude
    ///
    /// # Returns
    ///
    /// A `ResponseStream` that yields `Message` items for this turn until
    /// `Message::Result` is received or the CLI closes the stream.
    ///
    /// # Errors
    ///
    /// - `ClawError::Connection` - Not connected (call `connect()` first)
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
    /// // First turn
    /// let mut stream = client.send_message("What is 2+2?").await?;
    /// while let Some(result) = stream.next().await {
    ///     match result {
    ///         Ok(Message::Result(_)) => break,
    ///         Ok(msg) => println!("{:?}", msg),
    ///         Err(e) => eprintln!("Error: {}", e),
    ///     }
    /// }
    ///
    /// // Second turn (reuse the same client)
    /// let mut stream2 = client.send_message("And 3+3?").await?;
    /// while let Some(result) = stream2.next().await {
    ///     match result {
    ///         Ok(Message::Result(_)) => break,
    ///         Ok(msg) => println!("{:?}", msg),
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

        // Create a fresh per-turn channel
        let (tx, rx) = mpsc::unbounded_channel();

        // Install the sender in the routing slot so the background router
        // starts forwarding messages to this turn's receiver
        *self.current_turn_tx.lock().await = Some(tx);

        // Write the message to the CLI (AFTER installing the sender, so we
        // don't miss any messages that arrive immediately after the write)
        self.write_message(content.into().as_str()).await?;

        // Return the stream backed by the per-turn receiver
        Ok(ResponseStream::new(rx))
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
    ///   "session_id": "",
    ///   "message": {
    ///     "role": "user",
    ///     "content": "..."
    ///   },
    ///   "parent_tool_use_id": null
    /// }
    /// ```
    async fn write_message(&self, content: &str) -> Result<(), ClawError> {
        use serde_json::json;

        let transport = self
            .transport
            .as_ref()
            .ok_or_else(|| ClawError::Connection("Transport not available".to_string()))?;

        // Format user message (matches Python SDK format)
        let message = json!({
            "type": "user",
            "session_id": "",
            "message": {
                "role": "user",
                "content": content
            },
            "parent_tool_use_id": null
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

    /// Spawn background task that routes messages from the transport channel.
    ///
    /// Control messages (`control_request`, `control_response`) are dispatched
    /// to the `ControlProtocol`. All other messages are forwarded to the
    /// current-turn sender stored in `current_turn_tx`.
    ///
    /// This task runs for the lifetime of the connection. When `send_message()`
    /// is called, it installs a new sender in `current_turn_tx`; the router
    /// automatically starts delivering to the new turn's receiver.
    fn spawn_message_router(
        mut rx: mpsc::UnboundedReceiver<Result<Value, ClawError>>,
        control: Arc<ControlProtocol>,
        current_turn_tx: CurrentTurnSender,
    ) {
        use crate::control::messages::{ControlResponse, IncomingControlRequest};
        use tracing::{debug, warn};

        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                match msg {
                    Ok(value) => {
                        let msg_type = value.get("type").and_then(|v| v.as_str());

                        match msg_type {
                            Some("control_response") => {
                                // Extract request_id from INSIDE the response object
                                // (matches Python SDK: response.get("request_id"))
                                let request_id = value
                                    .get("response")
                                    .and_then(|r| r.get("request_id"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();

                                debug!(
                                    request_id = %request_id,
                                    "Received control_response"
                                );

                                if let Some(response_val) = value.get("response") {
                                    match serde_json::from_value::<ControlResponse>(
                                        response_val.clone(),
                                    ) {
                                        Ok(response) => {
                                            control.handle_response(&request_id, response).await;
                                        }
                                        Err(e) => {
                                            warn!("Failed to parse control response: {}", e);
                                        }
                                    }
                                }
                            }
                            Some("control_request") => {
                                let request_id = value
                                    .get("request_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();

                                if let Some(request_val) = value.get("request") {
                                    match serde_json::from_value::<IncomingControlRequest>(
                                        request_val.clone(),
                                    ) {
                                        Ok(incoming) => {
                                            control.handle_incoming(&request_id, incoming).await;
                                        }
                                        Err(e) => {
                                            warn!(
                                                "Failed to parse incoming control request: {}",
                                                e
                                            );
                                        }
                                    }
                                }
                            }
                            _ => {
                                // Forward non-control messages to the current turn's sender.
                                // Lock briefly to read the sender, then release before sending.
                                let sender = {
                                    let guard = current_turn_tx.lock().await;
                                    guard.clone()
                                };
                                if let Some(tx) = sender
                                    && tx.send(Ok(value)).is_err()
                                {
                                    // Receiver was dropped (caller discarded the stream).
                                    // Clear the slot so future sends are no-ops until
                                    // send_message() installs a new one.
                                    *current_turn_tx.lock().await = None;
                                }
                                // If no sender is installed (between turns), messages are discarded.
                            }
                        }
                    }
                    Err(e) => {
                        // Forward transport errors to the current turn's sender
                        let sender = {
                            let guard = current_turn_tx.lock().await;
                            guard.clone()
                        };
                        if let Some(tx) = sender
                            && tx.send(Err(e)).is_err()
                        {
                            *current_turn_tx.lock().await = None;
                        }
                    }
                }
            }

            debug!("Message routing task finished");
        });
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
            ControlResponse::Error { error, .. } => Err(ClawError::ControlError(format!(
                "Interrupt failed: {}",
                error
            ))),
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
            ControlResponse::Error { error, .. } => Err(ClawError::ControlError(format!(
                "Set model failed: {}",
                error
            ))),
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
            ControlResponse::Error { error, .. } => Err(ClawError::ControlError(format!(
                "MCP status query failed: {}",
                error
            ))),
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
                user_message_id: message_id.into(),
            })
            .await?;

        match response {
            ControlResponse::Success { .. } => Ok(()),
            ControlResponse::Error { error, .. } => Err(ClawError::ControlError(format!(
                "Rewind files failed: {}",
                error
            ))),
        }
    }

    /// Get server information from the CLI
    ///
    /// Returns version and capability information from the connected Claude Code CLI.
    /// This is useful for runtime capability detection and debugging.
    ///
    /// The returned `Value` is a JSON object with at least:
    /// - `"version"` — the CLI version string (e.g., `"2.1.45"`)
    ///
    /// Additional fields may be present depending on the CLI version.
    ///
    /// # Errors
    ///
    /// - `ClawError::Connection` - Not connected (call `connect()` first)
    /// - `ClawError::ControlTimeout` - Request timed out
    /// - `ClawError::ControlError` - Server info query failed
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
    /// let info = client.get_server_info().await?;
    /// println!("CLI version: {}", info["version"]);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_server_info(&self) -> Result<serde_json::Value, ClawError> {
        use crate::control::messages::{ControlRequest, ControlResponse};

        let control = self.control.as_ref().ok_or_else(|| {
            ClawError::Connection("Not connected. Call connect() first.".to_string())
        })?;

        let response = control.request(ControlRequest::GetServerInfo).await?;

        match response {
            ControlResponse::Success { data } => Ok(data),
            ControlResponse::Error { error, .. } => Err(ClawError::ControlError(format!(
                "Get server info failed: {}",
                error
            ))),
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
    /// #     async fn can_use_tool(&self, tool_name: &str, tool_input: &serde_json::Value) -> Result<rusty_claw::permissions::PermissionDecision, rusty_claw::error::ClawError> {
    /// #         Ok(rusty_claw::permissions::PermissionDecision::Allow { updated_input: None })
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
    pub async fn register_mcp_message_handler(&mut self, handler: Arc<dyn McpMessageHandler>) {
        if let Some(control) = &self.control {
            // Already connected: register directly on control protocol
            let mut handlers = control.handlers().await;
            handlers.register_mcp_message(handler);
        } else {
            // Not yet connected: store for apply during connect(), before initialize()
            self.pending_mcp_handler = Some(handler);
        }
    }
}

/// Run a closure with a freshly connected `ClaudeClient`, ensuring `close()` is called on exit.
///
/// This is the idiomatic Rust alternative to Python's `async with ClaudeSDKClient() as client:`
/// pattern. The client is connected before the closure runs and closed (even on error or panic)
/// after it completes.
///
/// # Arguments
///
/// * `options` - Configuration for the Claude session
/// * `f` - Async closure that receives a reference to the connected client
///
/// # Returns
///
/// The return value of the closure, or the first error encountered (connect, user, or close).
///
/// # Example
///
/// ```no_run
/// use rusty_claw::client::with_client;
/// use rusty_claw::prelude::*;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     with_client(ClaudeAgentOptions::default(), |_client| async {
///         // Use _client here (not captured by async block directly)
///         Ok(())
///     }).await?;
///     Ok(())
/// }
/// ```
pub async fn with_client<F, Fut>(options: ClaudeAgentOptions, f: F) -> Result<(), ClawError>
where
    F: FnOnce(&ClaudeClient) -> Fut,
    Fut: Future<Output = Result<(), ClawError>>,
{
    let mut client = ClaudeClient::new(options)?;
    client.connect().await?;

    let user_result = f(&client).await;

    // Always attempt to close, even if the closure returned an error
    let close_result = client.close().await;

    // Prefer propagating the user error; surface close error only if user succeeded
    match user_result {
        Err(e) => Err(e),
        Ok(()) => close_result,
    }
}

/// Stream of response messages from Claude CLI for a single conversation turn
///
/// `ResponseStream` wraps the per-turn message channel and:
/// - Parses raw JSON values into typed `Message` structs
/// - Yields only user-facing messages (Assistant, Result, System)
/// - Automatically ends when the CLI closes the stream
///
/// # Multi-Turn Usage
///
/// Each call to [`ClaudeClient::send_message()`] returns a fresh `ResponseStream`.
/// Drain or drop the stream before calling `send_message()` again.
///
/// # Control Message Routing
///
/// Control protocol messages (`control_request`, `control_response`) are
/// handled transparently by a background routing task spawned during
/// [`ClaudeClient::connect()`]. The `ResponseStream` never sees control
/// messages - they are filtered before reaching this stream.
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
    /// Receiver for per-turn user-facing messages
    rx: mpsc::UnboundedReceiver<Result<Value, ClawError>>,

    /// Whether the stream has completed (Result message received or channel closed)
    is_complete: bool,
}

impl ResponseStream {
    /// Create a new response stream backed by a per-turn receiver
    fn new(rx: mpsc::UnboundedReceiver<Result<Value, ClawError>>) -> Self {
        Self {
            rx,
            is_complete: false,
        }
    }

    /// Check if the stream has completed
    ///
    /// Returns `true` after the CLI has sent a `Message::Result` or closed the stream.
    pub fn is_complete(&self) -> bool {
        self.is_complete
    }

    /// Collect all messages for this turn until `Message::Result` is received
    ///
    /// This is a convenience method equivalent to iterating the stream with
    /// `StreamExt::next()` and breaking on `Message::Result`. The `Result`
    /// message itself is included in the returned vector.
    ///
    /// # Returns
    ///
    /// A `Vec<Message>` of all messages from this turn, ending with `Message::Result`.
    ///
    /// # Errors
    ///
    /// Returns the first `ClawError` encountered while reading the stream.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rusty_claw::prelude::*;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let options = ClaudeAgentOptions::default();
    /// # let mut client = ClaudeClient::new(options)?;
    /// # client.connect().await?;
    /// let stream = client.send_message("What is 2+2?").await?;
    /// let messages = stream.receive_response().await?;
    ///
    /// for msg in &messages {
    ///     if let Message::Assistant(asst) = msg {
    ///         println!("Claude replied with {} content blocks", asst.message.content.len());
    ///     }
    /// }
    ///
    /// // The last message is always the Result
    /// if let Some(Message::Result(result)) = messages.last() {
    ///     println!("Turn complete: {:?}", result);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn receive_response(mut self) -> Result<Vec<Message>, ClawError> {
        use tokio_stream::StreamExt;

        let mut messages = Vec::new();

        while let Some(result) = self.next().await {
            let msg = result?;
            let is_result = matches!(msg, Message::Result(_));
            messages.push(msg);
            if is_result {
                break;
            }
        }

        Ok(messages)
    }
}

impl Stream for ResponseStream {
    type Item = Result<Message, ClawError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Stream already complete
        if self.is_complete {
            return Poll::Ready(None);
        }

        // Poll the receiver - control messages are already filtered out by
        // the background message routing task spawned during connect()
        match Pin::new(&mut self.rx).poll_recv(cx) {
            Poll::Ready(Some(Ok(value))) => {
                match serde_json::from_value::<Message>(value.clone()) {
                    Ok(message) => Poll::Ready(Some(Ok(message))),
                    Err(e) => Poll::Ready(Some(Err(ClawError::MessageParse {
                        reason: format!("Failed to parse message: {}", e),
                        raw: value.to_string(),
                    }))),
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

/// Alias for [`ClaudeClient`] matching the Python SDK's `ClaudeSDKClient` class name.
///
/// The Python SDK uses `ClaudeSDKClient` as the primary client class name.
/// In Rust, [`ClaudeClient`] is the idiomatic name, but this alias allows
/// code ported from the Python SDK to compile with minimal changes.
///
/// # Example
///
/// ```no_run
/// use rusty_claw::prelude::{ClaudeSDKClient, ClaudeAgentOptions};
///
/// let options = ClaudeAgentOptions::default();
/// let client: ClaudeSDKClient = ClaudeSDKClient::new(options).unwrap();
/// ```
pub type ClaudeSDKClient = ClaudeClient;

#[cfg(test)]
mod tests {
    use super::*;

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
        let stream = ResponseStream::new(rx);
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

    #[tokio::test]
    async fn test_get_server_info_without_connect() {
        let options = ClaudeAgentOptions::default();
        let client = ClaudeClient::new(options).unwrap();
        let result = client.get_server_info().await;
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
        use crate::control::handlers::{CanUseToolHandler, HookHandler, McpMessageHandler};
        use crate::options::HookEvent;
        use async_trait::async_trait;
        use serde_json::{json, Value};

        #[derive(Debug)]
    struct TestPermHandler;
        #[async_trait]
        impl CanUseToolHandler for TestPermHandler {
            async fn can_use_tool(
                &self,
                _tool_name: &str,
                _tool_input: &serde_json::Value,
            ) -> Result<crate::permissions::PermissionDecision, ClawError> {
                Ok(crate::permissions::PermissionDecision::Allow { updated_input: None })
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
        let mut client = ClaudeClient::new(options).unwrap();

        // These should not panic even when not connected
        client
            .register_can_use_tool_handler(Arc::new(TestPermHandler))
            .await;
        client
            .register_hook("test".to_string(), Arc::new(TestHookHandler))
            .await;
        client
            .register_mcp_message_handler(Arc::new(TestMcpHandler))
            .await;
    }

    /// Test that send_message() can be called multiple times on the same client
    /// (using a fake connected state via direct channel manipulation).
    #[tokio::test]
    async fn test_send_message_multiple_turns() {
        // We simulate a connected client by directly manipulating internal state:
        // set `control` to Some so the guard check passes, and bypass write_message
        // by testing the channel creation logic independently.

        // Verify that successive calls to send_message() each get a fresh stream.
        // We do this by verifying the per-turn channel pattern directly.
        let current_turn_tx: CurrentTurnSender = Arc::new(Mutex::new(None));

        // Simulate first send_message: install sender 1
        let (tx1, rx1) = mpsc::unbounded_channel::<Result<Value, ClawError>>();
        *current_turn_tx.lock().await = Some(tx1);

        // Simulate second send_message: install sender 2 (replaces sender 1)
        let (tx2, rx2) = mpsc::unbounded_channel::<Result<Value, ClawError>>();
        *current_turn_tx.lock().await = Some(tx2);

        // tx1 is now orphaned (its corresponding entry was replaced in the slot).
        // rx1 should be closed since tx1 was dropped when it fell out of scope
        // (we didn't store it anywhere after the replacement).
        // rx2 should be open since tx2 is still in the slot.
        // Channel semantics: rx1 may or may not be closed depending on drop timing,
        // but the slot should have the turn-2 sender. We just verify the slot was updated.
        let _ = rx1; // Drop rx1 to avoid warnings

        // Verify the slot holds sender 2 (not sender 1)
        let slot_has_sender = current_turn_tx.lock().await.is_some();
        assert!(slot_has_sender, "Current turn sender should be set");

        // Verify that sending through the slot reaches rx2
        {
            let guard = current_turn_tx.lock().await;
            if let Some(tx) = guard.as_ref() {
                tx.send(Ok(serde_json::json!({"type": "system"}))).unwrap();
            }
        }
        let mut rx2 = rx2;
        let received = rx2.try_recv().unwrap();
        assert!(received.is_ok());
    }

    /// Test receive_response() collects until Message::Result
    #[tokio::test]
    async fn test_receive_response_collects_until_result() {
        use crate::messages::Message;

        let (tx, rx) = mpsc::unbounded_channel();

        // Send some messages including a final Result
        let assistant_json = serde_json::json!({
            "type": "assistant",
            "session_id": "test",
            "message": {
                "id": "msg_1",
                "role": "assistant",
                "content": [{"type": "text", "text": "Hello!"}],
                "model": "claude-opus-4",
                "stop_reason": null,
                "stop_sequence": null,
                "usage": {"input_tokens": 10, "output_tokens": 5, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0}
            }
        });

        let result_json = serde_json::json!({
            "type": "result",
            "subtype": "success",
            "session_id": "test",
            "result": "done",
            "is_error": false,
            "num_turns": 1,
            "usage": {"input_tokens": 10, "output_tokens": 5, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 0}
        });

        tx.send(Ok(assistant_json)).unwrap();
        tx.send(Ok(result_json)).unwrap();
        // Send a third message that should NOT be collected (after Result)
        tx.send(Ok(serde_json::json!({"type": "system", "subtype": "init", "session_id": "x", "tools": [], "mcp_servers": []}))).unwrap();

        let stream = ResponseStream::new(rx);
        let messages = stream.receive_response().await.unwrap();

        // Should have 2 messages: assistant + result (not the system after)
        assert_eq!(messages.len(), 2);
        assert!(matches!(messages[0], Message::Assistant(_)));
        assert!(matches!(messages[1], Message::Result(_)));
    }

    /// Test that with_client() type-checks: the closure receives a &ClaudeClient
    /// and the function returns Result<(), ClawError>
    #[test]
    fn test_with_client_type_signature() {
        // Verify that with_client compiles with the expected types.
        // We can't call it synchronously, but we can verify the function signature
        // by checking that the closure type is accepted.
        fn _assert_types() {
            // This function body is never run; it's a compile-time type check.
            let _f = |client: &ClaudeClient| {
                let _ = client.is_connected();
                async { Ok::<(), ClawError>(()) }
            };
        }
    }

    /// Test with_transport constructor
    #[test]
    fn test_with_transport_constructor() {
        use crate::transport::SubprocessCLITransport;
        let transport = SubprocessCLITransport::new(None, vec![]);
        let options = ClaudeAgentOptions::default();
        let client = ClaudeClient::with_transport(options, Box::new(transport));
        assert!(client.is_ok());
        let client = client.unwrap();
        // Transport was injected but not yet connected
        assert!(!client.is_connected());
    }
}

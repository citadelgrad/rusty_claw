//! Control protocol for bidirectional communication with Claude CLI
//!
//! This module implements the control protocol that enables:
//! - **Request/response routing** - Send requests to CLI and await responses
//! - **Handler dispatch** - Route incoming requests to registered handlers
//! - **Initialization handshake** - Configure the session with hooks, agents, and MCP servers
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                      ControlProtocol                        │
//! │                                                             │
//! │  ┌─────────────────────────────────────────────────────┐  │
//! │  │               Request/Response Router                │  │
//! │  │  - request() sends and awaits response              │  │
//! │  │  - handle_incoming() routes to handlers             │  │
//! │  └─────────────────────────────────────────────────────┘  │
//! │                          ↕                                  │
//! │  ┌──────────────────────┐      ┌─────────────────────┐   │
//! │  │  Pending Requests     │      │   Handlers          │   │
//! │  │  HashMap<String,      │      │   CanUseTool        │   │
//! │  │    oneshot::Sender>   │      │   HookCallbacks     │   │
//! │  │                       │      │   McpMessage        │   │
//! │  └──────────────────────┘      └─────────────────────┘   │
//! │                          ↕                                  │
//! │  ┌─────────────────────────────────────────────────────┐  │
//! │  │              Transport (Arc<dyn Transport>)         │  │
//! │  │  - write() sends messages to CLI stdin             │  │
//! │  │  - messages() receives from CLI stdout             │  │
//! │  └─────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```no_run
//! use rusty_claw::prelude::*;
//! use std::sync::Arc;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create and connect transport
//! let mut transport = SubprocessCLITransport::new(None, vec![]);
//! transport.connect().await?;
//! let transport = Arc::new(transport);
//!
//! // Create control protocol
//! let control = ControlProtocol::new(transport);
//!
//! // Initialize the session
//! let options = ClaudeAgentOptions::default();
//! control.initialize(&options).await?;
//!
//! // Send a control request
//! let response = control.request(ControlRequest::McpStatus).await?;
//! # Ok(())
//! # }
//! ```

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::control::handlers::ControlHandlers;
use crate::control::messages::{ControlRequest, ControlResponse, IncomingControlRequest};
use crate::control::pending::PendingRequests;
use crate::error::ClawError;
use crate::options::ClaudeAgentOptions;
use crate::transport::Transport;

pub mod handlers;
pub mod messages;
pub mod pending;

/// Control protocol for bidirectional communication with Claude CLI
///
/// The `ControlProtocol` manages:
/// - **Outgoing requests** - SDK → CLI control messages with response tracking
/// - **Incoming requests** - CLI → SDK control messages with handler dispatch
/// - **Initialization** - Session setup with hooks, agents, and MCP servers
///
/// # Thread Safety
///
/// `ControlProtocol` is `Send + Sync` and can be safely shared across tasks.
/// All internal state is protected by appropriate synchronization primitives.
///
/// # Example
///
/// ```no_run
/// use rusty_claw::prelude::*;
/// use std::sync::Arc;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut transport = SubprocessCLITransport::new(None, vec![]);
/// transport.connect().await?;
/// let transport = Arc::new(transport);
/// let control = ControlProtocol::new(transport);
///
/// // Send a control request
/// let response = control.request(ControlRequest::Interrupt).await?;
/// # Ok(())
/// # }
/// ```
pub struct ControlProtocol {
    /// Transport for sending/receiving messages
    transport: Arc<dyn Transport>,

    /// Pending outgoing requests awaiting responses
    pending: PendingRequests,

    /// Registered handlers for incoming requests
    handlers: Arc<Mutex<ControlHandlers>>,
}

impl ControlProtocol {
    /// Create a new control protocol instance
    ///
    /// # Arguments
    ///
    /// * `transport` - Transport layer for communication with CLI
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rusty_claw::prelude::*;
    /// use std::sync::Arc;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut transport = SubprocessCLITransport::new(None, vec![]);
    /// transport.connect().await?;
    /// let transport = Arc::new(transport);
    /// let control = ControlProtocol::new(transport);
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(transport: Arc<dyn Transport>) -> Self {
        Self {
            transport,
            pending: PendingRequests::new(),
            handlers: Arc::new(Mutex::new(ControlHandlers::new())),
        }
    }

    /// Get a mutable reference to the handler registry
    ///
    /// Use this to register handlers for can_use_tool, hooks, and MCP messages.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rusty_claw::prelude::*;
    /// use async_trait::async_trait;
    /// use std::sync::Arc;
    ///
    /// struct MyHandler;
    ///
    /// #[async_trait]
    /// impl CanUseToolHandler for MyHandler {
    ///     async fn can_use_tool(
    ///         &self,
    ///         _tool_name: &str,
    ///         _tool_input: &serde_json::Value,
    ///     ) -> Result<bool, ClawError> {
    ///         Ok(true)
    ///     }
    /// }
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut transport = SubprocessCLITransport::new(None, vec![]);
    /// transport.connect().await?;
    /// let transport = Arc::new(transport);
    /// let control = ControlProtocol::new(transport);
    ///
    /// let mut handlers = control.handlers().await;
    /// handlers.register_can_use_tool(Arc::new(MyHandler));
    /// # Ok(())
    /// # }
    /// ```
    pub async fn handlers(&self) -> tokio::sync::MutexGuard<'_, ControlHandlers> {
        self.handlers.lock().await
    }

    /// Initialize the agent session
    ///
    /// Sends an `initialize` control request to the CLI with configuration
    /// from `ClaudeAgentOptions`. This must be called before the CLI can
    /// process user messages.
    ///
    /// # Arguments
    ///
    /// * `options` - Session configuration (hooks, agents, MCP servers, etc.)
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Initialization successful
    /// * `Err(ClawError::ControlError)` - CLI returned an error
    /// * `Err(ClawError::ControlTimeout)` - CLI did not respond in time
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rusty_claw::prelude::*;
    /// use std::sync::Arc;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut transport = SubprocessCLITransport::new(None, vec![]);
    /// transport.connect().await?;
    /// let transport = Arc::new(transport);
    /// let control = ControlProtocol::new(transport);
    ///
    /// let options = ClaudeAgentOptions::builder()
    ///     .max_turns(5)
    ///     .build();
    ///
    /// control.initialize(&options).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn initialize(&self, options: &ClaudeAgentOptions) -> Result<(), ClawError> {
        let request = ControlRequest::Initialize {
            hooks: options.hooks.clone(),
            agents: options.agents.clone(),
            sdk_mcp_servers: options.sdk_mcp_servers.clone(),
            permissions: options.permission_mode.clone(),
            can_use_tool: true, // Enable can_use_tool callbacks
        };

        match self.request(request).await? {
            ControlResponse::Success { .. } => Ok(()),
            ControlResponse::Error { error, .. } => {
                Err(ClawError::ControlError(format!(
                    "Initialization failed: {}",
                    error
                )))
            }
        }
    }

    /// Send a control request and wait for the response
    ///
    /// Generates a unique request ID, sends the request to the CLI, and waits
    /// up to 30 seconds for a response. The response is delivered via the
    /// [`handle_response`](Self::handle_response) method.
    ///
    /// # Arguments
    ///
    /// * `request` - Control request to send
    ///
    /// # Returns
    ///
    /// * `Ok(ControlResponse)` - CLI responded successfully
    /// * `Err(ClawError::ControlTimeout)` - CLI did not respond within 30 seconds
    /// * `Err(ClawError::ControlError)` - Response channel was closed
    /// * `Err(ClawError::Connection)` - Failed to write request to CLI
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rusty_claw::prelude::*;
    /// use std::sync::Arc;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut transport = SubprocessCLITransport::new(None, vec![]);
    /// transport.connect().await?;
    /// let transport = Arc::new(transport);
    /// let control = ControlProtocol::new(transport);
    ///
    /// let response = control.request(ControlRequest::McpStatus).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn request(&self, request: ControlRequest) -> Result<ControlResponse, ClawError> {
        let id = Uuid::new_v4().to_string();
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.pending.insert(id.clone(), tx).await;

        // Construct the control_request message
        let msg = serde_json::json!({
            "type": "control_request",
            "request_id": id,
            "request": request,
        });

        // Send to CLI (NDJSON requires trailing newline)
        let mut bytes = serde_json::to_vec(&msg)?;
        bytes.push(b'\n');
        self.transport
            .write(&bytes)
            .await
            .map_err(|e| ClawError::Connection(format!("Failed to send control request: {}", e)))?;

        // Wait for response with timeout
        match tokio::time::timeout(Duration::from_secs(30), rx).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(_)) => Err(ClawError::ControlError(
                "Response channel closed".to_string(),
            )),
            Err(_) => {
                // Timeout - clean up pending entry
                self.pending.cancel(&id).await;
                Err(ClawError::ControlTimeout {
                    subtype: "control_request".to_string(),
                })
            }
        }
    }

    /// Handle a control response from the CLI
    ///
    /// Routes the response to the waiting `request()` caller via the oneshot channel.
    /// Called by the message receiver loop when a `control_response` message arrives.
    ///
    /// # Arguments
    ///
    /// * `request_id` - UUID of the original request
    /// * `response` - Response from the CLI
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rusty_claw::prelude::*;
    /// use serde_json::json;
    /// use std::sync::Arc;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut transport = SubprocessCLITransport::new(None, vec![]);
    /// transport.connect().await?;
    /// let transport = Arc::new(transport);
    /// let control = ControlProtocol::new(transport);
    ///
    /// // Called by message receiver when response arrives
    /// let response = ControlResponse::Success { data: json!({}) };
    /// control.handle_response("req_123", response).await;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn handle_response(&self, request_id: &str, response: ControlResponse) {
        self.pending.complete(request_id, response).await;
    }

    /// Handle an incoming control request from the CLI
    ///
    /// Routes the request to the appropriate registered handler:
    /// - **can_use_tool** → [`CanUseToolHandler`](handlers::CanUseToolHandler)
    /// - **hook_callback** → [`HookHandler`](handlers::HookHandler)
    /// - **mcp_message** → [`McpMessageHandler`](handlers::McpMessageHandler)
    ///
    /// If no handler is registered:
    /// - **can_use_tool**: Allow all tools (default: permissive)
    /// - **hook_callback**: Return error
    /// - **mcp_message**: Return error
    ///
    /// The response is sent back to the CLI as a `control_response` message.
    ///
    /// # Arguments
    ///
    /// * `request_id` - UUID of the incoming request
    /// * `request` - Incoming control request from CLI
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rusty_claw::prelude::*;
    /// use serde_json::json;
    /// use std::sync::Arc;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut transport = SubprocessCLITransport::new(None, vec![]);
    /// transport.connect().await?;
    /// let transport = Arc::new(transport);
    /// let control = ControlProtocol::new(transport);
    ///
    /// // Called by message receiver when incoming request arrives
    /// let request = IncomingControlRequest::CanUseTool {
    ///     tool_name: "Bash".to_string(),
    ///     tool_input: json!({ "command": "ls" }),
    /// };
    /// control.handle_incoming("req_123", request).await;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn handle_incoming(&self, request_id: &str, request: IncomingControlRequest) {
        use serde_json::json;
        use tracing::error;

        let response = match request {
            IncomingControlRequest::CanUseTool {
                tool_name,
                tool_input,
            } => {
                // Clone handler Arc and drop lock before awaiting to avoid deadlock
                let handler = {
                    let handlers = self.handlers.lock().await;
                    handlers.can_use_tool.clone()
                };
                if let Some(handler) = handler {
                    match handler.can_use_tool(&tool_name, &tool_input).await {
                        Ok(allowed) => ControlResponse::Success {
                            data: json!({ "allowed": allowed }),
                        },
                        Err(e) => ControlResponse::Error {
                            error: e.to_string(),
                            extra: json!({}),
                        },
                    }
                } else {
                    // Default: allow all tools
                    ControlResponse::Success {
                        data: json!({ "allowed": true }),
                    }
                }
            }

            IncomingControlRequest::HookCallback {
                hook_id,
                hook_event,
                hook_input,
            } => {
                // Clone handler Arc and drop lock before awaiting to avoid deadlock
                let handler = {
                    let handlers = self.handlers.lock().await;
                    handlers.hook_callbacks.get(&hook_id).cloned()
                };
                if let Some(handler) = handler {
                    match handler.call(hook_event, hook_input).await {
                        Ok(result) => ControlResponse::Success { data: result },
                        Err(e) => ControlResponse::Error {
                            error: e.to_string(),
                            extra: json!({}),
                        },
                    }
                } else {
                    ControlResponse::Error {
                        error: format!("No handler registered for hook_id: {}", hook_id),
                        extra: json!({}),
                    }
                }
            }

            IncomingControlRequest::McpMessage {
                server_name,
                message,
            } => {
                // Clone handler Arc and drop lock before awaiting to avoid deadlock
                let handler = {
                    let handlers = self.handlers.lock().await;
                    handlers.mcp_message.clone()
                };
                if let Some(handler) = handler {
                    match handler.handle(&server_name, message).await {
                        Ok(result) => ControlResponse::Success { data: result },
                        Err(e) => ControlResponse::Error {
                            error: e.to_string(),
                            extra: json!({}),
                        },
                    }
                } else {
                    ControlResponse::Error {
                        error: "No MCP message handler registered".to_string(),
                        extra: json!({}),
                    }
                }
            }
        };

        // Send response back to CLI
        let msg = json!({
            "type": "control_response",
            "request_id": request_id,
            "response": response,
        });

        match serde_json::to_vec(&msg) {
            Ok(mut bytes) => {
                bytes.push(b'\n'); // NDJSON requires trailing newline
                if let Err(e) = self.transport.write(&bytes).await {
                    error!("Failed to send control response: {}", e);
                }
            }
            Err(e) => {
                error!("Failed to serialize control response: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::handlers::{CanUseToolHandler, HookHandler, McpMessageHandler};
    use crate::options::HookEvent;
    use async_trait::async_trait;
    use serde_json::{json, Value};
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use tokio::sync::Mutex as TokioMutex;

    // Mock transport for testing
    struct MockTransport {
        sent: Arc<TokioMutex<Vec<Vec<u8>>>>,
        receiver: Arc<TokioMutex<Option<mpsc::UnboundedReceiver<Result<Value, ClawError>>>>>,
        sender: mpsc::UnboundedSender<Result<Value, ClawError>>,
    }

    impl MockTransport {
        fn new() -> Self {
            let (sender, receiver) = mpsc::unbounded_channel();
            Self {
                sent: Arc::new(TokioMutex::new(Vec::new())),
                receiver: Arc::new(TokioMutex::new(Some(receiver))),
                sender,
            }
        }

        async fn get_sent(&self) -> Vec<Vec<u8>> {
            self.sent.lock().await.clone()
        }

        fn simulate_response(&self, request_id: &str, response: ControlResponse) {
            let msg = json!({
                "type": "control_response",
                "request_id": request_id,
                "response": response,
            });
            self.sender.send(Ok(msg)).unwrap();
        }
    }

    #[async_trait]
    impl Transport for MockTransport {
        async fn connect(&mut self) -> Result<(), ClawError> {
            Ok(())
        }

        async fn write(&self, data: &[u8]) -> Result<(), ClawError> {
            self.sent.lock().await.push(data.to_vec());
            Ok(())
        }

        fn messages(&self) -> mpsc::UnboundedReceiver<Result<Value, ClawError>> {
            // SAFETY: This is a test mock. We use blocking_lock which is safe
            // in test contexts where we control the async runtime.
            self.receiver.blocking_lock().take().unwrap()
        }

        async fn end_input(&self) -> Result<(), ClawError> {
            Ok(())
        }

        async fn close(&mut self) -> Result<(), ClawError> {
            Ok(())
        }

        fn is_ready(&self) -> bool {
            true
        }
    }

    // Mock handlers
    struct MockCanUseToolHandler;

    #[async_trait]
    impl CanUseToolHandler for MockCanUseToolHandler {
        async fn can_use_tool(
            &self,
            tool_name: &str,
            _tool_input: &Value,
        ) -> Result<bool, ClawError> {
            Ok(tool_name == "Read")
        }
    }

    struct MockHookHandler;

    #[async_trait]
    impl HookHandler for MockHookHandler {
        async fn call(&self, _hook_event: HookEvent, hook_input: Value) -> Result<Value, ClawError> {
            Ok(json!({ "echo": hook_input }))
        }
    }

    struct MockMcpHandler;

    #[async_trait]
    impl McpMessageHandler for MockMcpHandler {
        async fn handle(&self, server_name: &str, _message: Value) -> Result<Value, ClawError> {
            Ok(json!({ "server": server_name }))
        }
    }

    #[tokio::test]
    async fn test_request_success() {
        let transport = Arc::new(MockTransport::new());
        let control = ControlProtocol::new(transport.clone() as Arc<dyn Transport>);
        let control_clone = Arc::new(control);

        // Spawn a task to simulate CLI response by monitoring sent messages
        let transport_clone = transport.clone();
        let control_for_response = control_clone.clone();
        tokio::spawn(async move {
            // Wait a bit for the request to be sent
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            let sent = transport_clone.get_sent().await;
            if sent.is_empty() {
                return;
            }

            let msg: Value = serde_json::from_slice(&sent[0]).unwrap();
            let request_id = msg["request_id"].as_str().unwrap().to_string();

            // Simulate CLI response by directly calling handle_response
            control_for_response.handle_response(
                &request_id,
                ControlResponse::Success {
                    data: json!({ "result": "ok" }),
                },
            ).await;
        });

        let response = control_clone.request(ControlRequest::Interrupt).await.unwrap();

        match response {
            ControlResponse::Success { data } => {
                assert_eq!(data["result"], "ok");
            }
            _ => panic!("Expected success response"),
        }
    }

    #[tokio::test]
    async fn test_initialize_success() {
        let transport = Arc::new(MockTransport::new());
        let control = ControlProtocol::new(transport.clone() as Arc<dyn Transport>);
        let control_clone = Arc::new(control);

        // Spawn a task to simulate CLI response
        let transport_clone = transport.clone();
        let control_for_response = control_clone.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            let sent = transport_clone.get_sent().await;
            if sent.is_empty() {
                return;
            }

            let msg: Value = serde_json::from_slice(&sent[0]).unwrap();
            let request_id = msg["request_id"].as_str().unwrap().to_string();

            control_for_response.handle_response(
                &request_id,
                ControlResponse::Success { data: json!({}) },
            ).await;
        });

        let options = ClaudeAgentOptions::default();
        control_clone.initialize(&options).await.unwrap();
    }

    #[tokio::test]
    async fn test_initialize_error() {
        let transport = Arc::new(MockTransport::new());
        let control = ControlProtocol::new(transport.clone() as Arc<dyn Transport>);
        let control_clone = Arc::new(control);

        // Spawn a task to simulate CLI error response
        let transport_clone = transport.clone();
        let control_for_response = control_clone.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            let sent = transport_clone.get_sent().await;
            if sent.is_empty() {
                return;
            }

            let msg: Value = serde_json::from_slice(&sent[0]).unwrap();
            let request_id = msg["request_id"].as_str().unwrap().to_string();

            control_for_response.handle_response(
                &request_id,
                ControlResponse::Error {
                    error: "Bad config".to_string(),
                    extra: json!({}),
                },
            ).await;
        });

        let options = ClaudeAgentOptions::default();
        let result = control_clone.initialize(&options).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Initialization failed"));
    }

    #[tokio::test]
    async fn test_handle_incoming_can_use_tool_with_handler() {
        let transport = Arc::new(MockTransport::new());
        let control = ControlProtocol::new(transport.clone() as Arc<dyn Transport>);

        // Register handler
        {
            let mut handlers = control.handlers().await;
            handlers.register_can_use_tool(Arc::new(MockCanUseToolHandler));
        }

        // Handle incoming request
        let request = IncomingControlRequest::CanUseTool {
            tool_name: "Read".to_string(),
            tool_input: json!({}),
        };
        control.handle_incoming("req_1", request).await;

        // Check sent response
        let sent = transport.get_sent().await;
        assert_eq!(sent.len(), 1);
        let msg: Value = serde_json::from_slice(&sent[0]).unwrap();
        assert_eq!(msg["type"], "control_response");
        assert_eq!(msg["response"]["subtype"], "success");
        assert_eq!(msg["response"]["allowed"], true);
    }

    #[tokio::test]
    async fn test_handle_incoming_can_use_tool_default() {
        let transport = Arc::new(MockTransport::new());
        let control = ControlProtocol::new(transport.clone() as Arc<dyn Transport>);

        // No handler registered - should allow by default
        let request = IncomingControlRequest::CanUseTool {
            tool_name: "Bash".to_string(),
            tool_input: json!({}),
        };
        control.handle_incoming("req_1", request).await;

        let sent = transport.get_sent().await;
        let msg: Value = serde_json::from_slice(&sent[0]).unwrap();
        assert_eq!(msg["response"]["allowed"], true);
    }

    #[tokio::test]
    async fn test_handle_incoming_hook_callback() {
        let transport = Arc::new(MockTransport::new());
        let control = ControlProtocol::new(transport.clone() as Arc<dyn Transport>);

        // Register handler
        {
            let mut handlers = control.handlers().await;
            handlers.register_hook("hook1".to_string(), Arc::new(MockHookHandler));
        }

        // Handle incoming request
        let request = IncomingControlRequest::HookCallback {
            hook_id: "hook1".to_string(),
            hook_event: crate::options::HookEvent::PreToolUse,
            hook_input: json!({ "test": "data" }),
        };
        control.handle_incoming("req_1", request).await;

        let sent = transport.get_sent().await;
        let msg: Value = serde_json::from_slice(&sent[0]).unwrap();
        assert_eq!(msg["response"]["subtype"], "success");
        assert_eq!(msg["response"]["echo"]["test"], "data");
    }

    #[tokio::test]
    async fn test_handle_incoming_mcp_message() {
        let transport = Arc::new(MockTransport::new());
        let control = ControlProtocol::new(transport.clone() as Arc<dyn Transport>);

        // Register handler
        {
            let mut handlers = control.handlers().await;
            handlers.register_mcp_message(Arc::new(MockMcpHandler));
        }

        // Handle incoming request
        let request = IncomingControlRequest::McpMessage {
            server_name: "test_server".to_string(),
            message: json!({ "method": "test" }),
        };
        control.handle_incoming("req_1", request).await;

        let sent = transport.get_sent().await;
        let msg: Value = serde_json::from_slice(&sent[0]).unwrap();
        assert_eq!(msg["response"]["subtype"], "success");
        assert_eq!(msg["response"]["server"], "test_server");
    }
}

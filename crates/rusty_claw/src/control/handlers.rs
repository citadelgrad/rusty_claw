//! Handler traits and registry for control protocol callbacks
//!
//! This module defines the handler traits that SDK users can implement
//! to respond to incoming control requests from the Claude CLI:
//!
//! - [`CanUseToolHandler`] - Control tool execution permissions
//! - [`HookHandler`] - Execute custom hooks on events
//! - [`McpMessageHandler`] - Route MCP JSON-RPC messages to SDK servers
//!
//! # Example
//!
//! ```
//! use rusty_claw::control::handlers::{CanUseToolHandler, ControlHandlers};
//! use rusty_claw::error::ClawError;
//! use async_trait::async_trait;
//! use std::sync::Arc;
//!
//! struct MyToolHandler;
//!
//! #[async_trait]
//! impl CanUseToolHandler for MyToolHandler {
//!     async fn can_use_tool(
//!         &self,
//!         tool_name: &str,
//!         _tool_input: &serde_json::Value,
//!     ) -> Result<bool, ClawError> {
//!         // Only allow Read and Grep tools
//!         Ok(matches!(tool_name, "Read" | "Grep"))
//!     }
//! }
//!
//! let mut handlers = ControlHandlers::new();
//! handlers.register_can_use_tool(Arc::new(MyToolHandler));
//! ```

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::error::ClawError;
use crate::options::HookEvent;

/// Handler for can_use_tool permission callbacks
///
/// Implement this trait to control which tools the Claude CLI can execute.
/// The handler is invoked before each tool use, allowing the SDK to:
/// - Block dangerous or expensive tools
/// - Log tool usage
/// - Apply custom permission logic
/// - Prompt the user for confirmation
///
/// # Default Behavior
///
/// If no handler is registered, all tools are allowed by default.
///
/// # Example
///
/// ```
/// use rusty_claw::control::handlers::CanUseToolHandler;
/// use rusty_claw::error::ClawError;
/// use async_trait::async_trait;
///
/// struct AllowReadOnlyTools;
///
/// #[async_trait]
/// impl CanUseToolHandler for AllowReadOnlyTools {
///     async fn can_use_tool(
///         &self,
///         tool_name: &str,
///         _tool_input: &serde_json::Value,
///     ) -> Result<bool, ClawError> {
///         // Only allow read-only tools
///         Ok(matches!(tool_name, "Read" | "Grep" | "Glob"))
///     }
/// }
/// ```
#[async_trait]
pub trait CanUseToolHandler: Send + Sync {
    /// Check if a tool should be allowed to execute
    ///
    /// # Arguments
    ///
    /// * `tool_name` - Name of the tool being invoked (e.g., "Bash", "Read")
    /// * `tool_input` - Tool input parameters as JSON
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Allow tool execution
    /// * `Ok(false)` - Deny tool execution
    /// * `Err(...)` - Handler error (tool will be denied)
    async fn can_use_tool(
        &self,
        tool_name: &str,
        tool_input: &Value,
    ) -> Result<bool, ClawError>;
}

/// Handler for hook callbacks
///
/// Implement this trait to execute custom logic in response to agent events.
/// Hooks are registered during initialization and invoked by the CLI when
/// matching events occur.
///
/// # Example
///
/// ```
/// use rusty_claw::control::handlers::HookHandler;
/// use rusty_claw::error::ClawError;
/// use rusty_claw::options::HookEvent;
/// use async_trait::async_trait;
/// use serde_json::{json, Value};
///
/// struct LoggingHook;
///
/// #[async_trait]
/// impl HookHandler for LoggingHook {
///     async fn call(
///         &self,
///         _hook_event: HookEvent,
///         hook_input: Value,
///     ) -> Result<Value, ClawError> {
///         println!("Hook triggered: {:?}", hook_input);
///         Ok(json!({ "status": "logged" }))
///     }
/// }
/// ```
#[async_trait]
pub trait HookHandler: Send + Sync {
    /// Execute the hook callback
    ///
    /// # Arguments
    ///
    /// * `hook_event` - Event that triggered this hook
    /// * `hook_input` - Event data and context as JSON
    ///
    /// # Returns
    ///
    /// * `Ok(Value)` - Hook result data (returned to CLI)
    /// * `Err(...)` - Hook execution error
    async fn call(&self, hook_event: HookEvent, hook_input: Value) -> Result<Value, ClawError>;
}

/// Handler for MCP message routing
///
/// Implement this trait to route JSON-RPC messages from the Claude CLI
/// to SDK-hosted MCP servers. The handler receives the message, dispatches
/// it to the appropriate server, and returns the JSON-RPC response.
///
/// # Example
///
/// ```
/// use rusty_claw::control::handlers::McpMessageHandler;
/// use rusty_claw::error::ClawError;
/// use async_trait::async_trait;
/// use serde_json::{json, Value};
///
/// struct MyMcpRouter;
///
/// #[async_trait]
/// impl McpMessageHandler for MyMcpRouter {
///     async fn handle(
///         &self,
///         server_name: &str,
///         message: Value,
///     ) -> Result<Value, ClawError> {
///         // Route to appropriate MCP server
///         match server_name {
///             "my_tool_server" => {
///                 // Handle the JSON-RPC request
///                 Ok(json!({
///                     "jsonrpc": "2.0",
///                     "id": message["id"],
///                     "result": { "content": [{ "type": "text", "text": "Done" }] }
///                 }))
///             }
///             _ => Err(ClawError::ControlError(format!(
///                 "Unknown MCP server: {}",
///                 server_name
///             ))),
///         }
///     }
/// }
/// ```
#[async_trait]
pub trait McpMessageHandler: Send + Sync {
    /// Route an MCP message to the appropriate SDK server
    ///
    /// # Arguments
    ///
    /// * `server_name` - Name of the SDK-hosted MCP server
    /// * `message` - JSON-RPC message from the CLI
    ///
    /// # Returns
    ///
    /// * `Ok(Value)` - JSON-RPC response
    /// * `Err(...)` - Routing or execution error
    async fn handle(&self, server_name: &str, message: Value) -> Result<Value, ClawError>;
}

/// Registry for control protocol handlers
///
/// Stores registered handlers for can_use_tool, hooks, and MCP messages.
/// Handlers are optional - if no handler is registered, default behavior applies:
///
/// - **can_use_tool**: Allow all tools by default
/// - **hooks**: Return error if hook is invoked
/// - **mcp_message**: Return error if MCP message is received
///
/// # Example
///
/// ```
/// use rusty_claw::control::handlers::ControlHandlers;
/// use std::sync::Arc;
///
/// let mut handlers = ControlHandlers::new();
/// // handlers.register_can_use_tool(Arc::new(my_handler));
/// // handlers.register_hook("hook_id".to_string(), Arc::new(my_hook));
/// ```
#[derive(Default)]
pub struct ControlHandlers {
    /// Handler for can_use_tool permission checks
    pub(crate) can_use_tool: Option<Arc<dyn CanUseToolHandler>>,

    /// Handlers for hook callbacks, keyed by hook_id
    pub(crate) hook_callbacks: HashMap<String, Arc<dyn HookHandler>>,

    /// Handler for MCP message routing
    pub(crate) mcp_message: Option<Arc<dyn McpMessageHandler>>,
}

impl ControlHandlers {
    /// Create a new empty handler registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a handler for can_use_tool permission checks
    ///
    /// This handler will be invoked before each tool execution to determine
    /// if the tool should be allowed to run.
    ///
    /// # Example
    ///
    /// ```
    /// use rusty_claw::control::handlers::{CanUseToolHandler, ControlHandlers};
    /// use rusty_claw::error::ClawError;
    /// use async_trait::async_trait;
    /// use std::sync::Arc;
    ///
    /// struct MyHandler;
    ///
    /// #[async_trait]
    /// impl CanUseToolHandler for MyHandler {
    ///     async fn can_use_tool(
    ///         &self,
    ///         tool_name: &str,
    ///         _tool_input: &serde_json::Value,
    ///     ) -> Result<bool, ClawError> {
    ///         Ok(tool_name != "Bash")
    ///     }
    /// }
    ///
    /// let mut handlers = ControlHandlers::new();
    /// handlers.register_can_use_tool(Arc::new(MyHandler));
    /// ```
    pub fn register_can_use_tool(&mut self, handler: Arc<dyn CanUseToolHandler>) {
        self.can_use_tool = Some(handler);
    }

    /// Register a handler for a specific hook
    ///
    /// Multiple hooks can be registered with different hook_id values.
    /// When the CLI invokes a hook, it will call the handler with the matching hook_id.
    ///
    /// # Arguments
    ///
    /// * `hook_id` - Unique identifier for this hook (e.g., "pre_commit_hook")
    /// * `handler` - Implementation of the HookHandler trait
    ///
    /// # Example
    ///
    /// ```
    /// use rusty_claw::control::handlers::{HookHandler, ControlHandlers};
    /// use rusty_claw::error::ClawError;
    /// use rusty_claw::options::HookEvent;
    /// use async_trait::async_trait;
    /// use serde_json::{json, Value};
    /// use std::sync::Arc;
    ///
    /// struct MyHook;
    ///
    /// #[async_trait]
    /// impl HookHandler for MyHook {
    ///     async fn call(
    ///         &self,
    ///         _hook_event: HookEvent,
    ///         _hook_input: Value,
    ///     ) -> Result<Value, ClawError> {
    ///         Ok(json!({ "status": "ok" }))
    ///     }
    /// }
    ///
    /// let mut handlers = ControlHandlers::new();
    /// handlers.register_hook("my_hook".to_string(), Arc::new(MyHook));
    /// ```
    pub fn register_hook(&mut self, hook_id: String, handler: Arc<dyn HookHandler>) {
        self.hook_callbacks.insert(hook_id, handler);
    }

    /// Register a handler for MCP message routing
    ///
    /// This handler will receive all MCP JSON-RPC messages from the CLI
    /// and route them to the appropriate SDK-hosted MCP server.
    ///
    /// # Example
    ///
    /// ```
    /// use rusty_claw::control::handlers::{McpMessageHandler, ControlHandlers};
    /// use rusty_claw::error::ClawError;
    /// use async_trait::async_trait;
    /// use serde_json::Value;
    /// use std::sync::Arc;
    ///
    /// struct MyRouter;
    ///
    /// #[async_trait]
    /// impl McpMessageHandler for MyRouter {
    ///     async fn handle(
    ///         &self,
    ///         _server_name: &str,
    ///         _message: Value,
    ///     ) -> Result<Value, ClawError> {
    ///         Ok(serde_json::json!({}))
    ///     }
    /// }
    ///
    /// let mut handlers = ControlHandlers::new();
    /// handlers.register_mcp_message(Arc::new(MyRouter));
    /// ```
    pub fn register_mcp_message(&mut self, handler: Arc<dyn McpMessageHandler>) {
        self.mcp_message = Some(handler);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
    async fn test_can_use_tool_handler() {
        let handler = MockCanUseToolHandler;
        assert_eq!(
            handler.can_use_tool("Read", &json!({})).await.unwrap(),
            true
        );
        assert_eq!(
            handler.can_use_tool("Bash", &json!({})).await.unwrap(),
            false
        );
    }

    #[tokio::test]
    async fn test_hook_handler() {
        let handler = MockHookHandler;
        let result = handler.call(HookEvent, json!({ "foo": "bar" })).await.unwrap();
        assert_eq!(result["echo"]["foo"], "bar");
    }

    #[tokio::test]
    async fn test_mcp_handler() {
        let handler = MockMcpHandler;
        let result = handler
            .handle("test_server", json!({}))
            .await
            .unwrap();
        assert_eq!(result["server"], "test_server");
    }

    #[test]
    fn test_handlers_registry_default() {
        let handlers = ControlHandlers::new();
        assert!(handlers.can_use_tool.is_none());
        assert!(handlers.hook_callbacks.is_empty());
        assert!(handlers.mcp_message.is_none());
    }

    #[test]
    fn test_handlers_register_can_use_tool() {
        let mut handlers = ControlHandlers::new();
        handlers.register_can_use_tool(Arc::new(MockCanUseToolHandler));
        assert!(handlers.can_use_tool.is_some());
    }

    #[test]
    fn test_handlers_register_hook() {
        let mut handlers = ControlHandlers::new();
        handlers.register_hook("hook1".to_string(), Arc::new(MockHookHandler));
        handlers.register_hook("hook2".to_string(), Arc::new(MockHookHandler));
        assert_eq!(handlers.hook_callbacks.len(), 2);
        assert!(handlers.hook_callbacks.contains_key("hook1"));
        assert!(handlers.hook_callbacks.contains_key("hook2"));
    }

    #[test]
    fn test_handlers_register_mcp_message() {
        let mut handlers = ControlHandlers::new();
        handlers.register_mcp_message(Arc::new(MockMcpHandler));
        assert!(handlers.mcp_message.is_some());
    }
}

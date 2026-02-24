//! Model Context Protocol (MCP) server bridge for SDK-hosted tools
//!
//! This module implements the MCP server bridge that enables SDK users to register
//! Rust functions as tools invokable by Claude via the MCP protocol. The bridge handles:
//!
//! - **Tool Registration** - Register Rust functions as MCP tools
//! - **JSON-RPC Routing** - Route `initialize`, `tools/list`, and `tools/call` messages
//! - **Tool Execution** - Execute tools asynchronously with proper error handling
//! - **Result Formatting** - Convert tool results to MCP-compatible JSON
//!
//! # Architecture
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────────┐
//! │                     MCP Server Bridge                          │
//! │                                                                │
//! │  ┌──────────────────────────────────────────────────────────┐ │
//! │  │               SdkMcpServerRegistry                       │ │
//! │  │  (implements McpMessageHandler)                          │ │
//! │  │                                                          │ │
//! │  │  - Routes CLI messages to SdkMcpServer instances         │ │
//! │  │  - Manages multiple servers by name                      │ │
//! │  └──────────────────────────────────────────────────────────┘ │
//! │                          │                                     │
//! │                          │ Contains HashMap<String, Server>    │
//! │                          ▼                                     │
//! │  ┌──────────────────────────────────────────────────────────┐ │
//! │  │                 SdkMcpServerImpl                         │ │
//! │  │                                                          │ │
//! │  │  - Tool registry: HashMap<String, SdkMcpTool>            │ │
//! │  │  - JSON-RPC handler: handle_jsonrpc()                    │ │
//! │  │  - Methods: initialize, tools/list, tools/call           │ │
//! │  └──────────────────────────────────────────────────────────┘ │
//! │                          │                                     │
//! │                          │ Contains Vec<SdkMcpTool>            │
//! │                          ▼                                     │
//! │  ┌──────────────────────────────────────────────────────────┐ │
//! │  │                   SdkMcpTool                             │ │
//! │  │                                                          │ │
//! │  │  - Tool metadata (name, description, schema)             │ │
//! │  │  - Handler reference: Arc<dyn ToolHandler>               │ │
//! │  │  - execute() method delegates to handler                 │ │
//! │  └──────────────────────────────────────────────────────────┘ │
//! │                          │                                     │
//! │                          │ Uses Arc<dyn ToolHandler>           │
//! │                          ▼                                     │
//! │  ┌──────────────────────────────────────────────────────────┐ │
//! │  │           ToolHandler (async trait)                      │ │
//! │  │                                                          │ │
//! │  │  async fn call(&self, args: Value)                       │ │
//! │  │      → Result<ToolResult, ClawError>                     │ │
//! │  └──────────────────────────────────────────────────────────┘ │
//! └────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```
//! use rusty_claw::prelude::*;
//! use rusty_claw::mcp_server::{SdkMcpServerImpl, SdkMcpTool, ToolHandler, ToolResult};
//! use async_trait::async_trait;
//! use serde_json::{json, Value};
//! use std::sync::Arc;
//!
//! // Define a tool handler
//! struct CalculatorHandler;
//!
//! #[async_trait]
//! impl ToolHandler for CalculatorHandler {
//!     async fn call(&self, args: Value) -> Result<ToolResult, ClawError> {
//!         let a = args["a"].as_f64().unwrap_or(0.0);
//!         let b = args["b"].as_f64().unwrap_or(0.0);
//!         Ok(ToolResult::text(format!("Result: {}", a + b)))
//!     }
//! }
//!
//! // Create a tool
//! let tool = SdkMcpTool::new(
//!     "add",
//!     "Add two numbers",
//!     json!({
//!         "type": "object",
//!         "properties": {
//!             "a": { "type": "number" },
//!             "b": { "type": "number" }
//!         },
//!         "required": ["a", "b"]
//!     }),
//!     Arc::new(CalculatorHandler),
//! );
//!
//! // Create and register server
//! let mut server = SdkMcpServerImpl::new("calculator", "1.0.0");
//! server.register_tool(tool);
//! ```

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

use crate::control::handlers::McpMessageHandler;
use crate::error::ClawError;

/// Content type for tool results
///
/// MCP tools can return text or image content. This enum represents
/// the different content types that can be included in a tool result.
///
/// # Example
///
/// ```
/// use rusty_claw::mcp_server::ToolContent;
///
/// let text = ToolContent::text("Hello, world!");
/// let image = ToolContent::image("base64data", "image/png");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ToolContent {
    /// Text content
    Text {
        /// The text content
        text: String,
    },
    /// Image content
    Image {
        /// Base64-encoded image data
        data: String,
        /// MIME type (e.g., "image/png")
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
}

impl ToolContent {
    /// Create text content
    ///
    /// # Example
    ///
    /// ```
    /// use rusty_claw::mcp_server::ToolContent;
    ///
    /// let content = ToolContent::text("Hello!");
    /// ```
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    /// Create image content
    ///
    /// # Example
    ///
    /// ```
    /// use rusty_claw::mcp_server::ToolContent;
    ///
    /// let content = ToolContent::image("base64data", "image/png");
    /// ```
    pub fn image(data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self::Image {
            data: data.into(),
            mime_type: mime_type.into(),
        }
    }
}

/// Result of tool execution
///
/// Wraps tool output with error flag for MCP protocol responses.
/// Tool results can contain multiple content items (text, images, etc.)
/// and an error flag to indicate failure.
///
/// # Example
///
/// ```
/// use rusty_claw::mcp_server::ToolResult;
///
/// // Success result
/// let result = ToolResult::text("Operation completed");
///
/// // Error result
/// let error = ToolResult::error("Failed to process");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Content items returned by the tool
    pub content: Vec<ToolContent>,
    /// Whether this result represents an error
    #[serde(rename = "isError", skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

impl ToolResult {
    /// Create a text result
    ///
    /// # Example
    ///
    /// ```
    /// use rusty_claw::mcp_server::ToolResult;
    ///
    /// let result = ToolResult::text("Success");
    /// ```
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::text(text)],
            is_error: None,
        }
    }

    /// Create an error result
    ///
    /// # Example
    ///
    /// ```
    /// use rusty_claw::mcp_server::ToolResult;
    ///
    /// let result = ToolResult::error("Failed");
    /// ```
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::text(text)],
            is_error: Some(true),
        }
    }

    /// Create a result with multiple content items
    ///
    /// # Example
    ///
    /// ```
    /// use rusty_claw::mcp_server::{ToolResult, ToolContent};
    ///
    /// let result = ToolResult::new(vec![
    ///     ToolContent::text("Description"),
    ///     ToolContent::image("base64data", "image/png"),
    /// ]);
    /// ```
    pub fn new(content: Vec<ToolContent>) -> Self {
        Self {
            content,
            is_error: None,
        }
    }
}

/// Handler for tool execution
///
/// Implement this trait to define the behavior of an MCP tool.
/// The handler receives JSON arguments and returns a [`ToolResult`].
///
/// # Thread Safety
///
/// Handlers must be `Send + Sync` to support concurrent execution.
///
/// # Example
///
/// ```
/// use rusty_claw::prelude::*;
/// use rusty_claw::mcp_server::{ToolHandler, ToolResult};
/// use async_trait::async_trait;
/// use serde_json::Value;
///
/// struct EchoHandler;
///
/// #[async_trait]
/// impl ToolHandler for EchoHandler {
///     async fn call(&self, args: Value) -> Result<ToolResult, ClawError> {
///         let message = args["message"].as_str().unwrap_or("empty");
///         Ok(ToolResult::text(format!("Echo: {}", message)))
///     }
/// }
/// ```
#[async_trait]
pub trait ToolHandler: Send + Sync {
    /// Execute the tool with the given arguments
    ///
    /// # Arguments
    ///
    /// * `args` - JSON object containing tool arguments
    ///
    /// # Returns
    ///
    /// * `Ok(ToolResult)` - Tool output
    /// * `Err(ClawError)` - Execution error
    async fn call(&self, args: Value) -> Result<ToolResult, ClawError>;
}

/// Type-safe wrapper for tool handlers with automatic JSON deserialization
///
/// `TypedToolHandler<I>` wraps a closure (or async function) that accepts a
/// concrete Rust type `I` instead of raw [`serde_json::Value`]. The wrapper
/// automatically deserializes the incoming `args: Value` into `I` before
/// calling the inner handler, providing descriptive error messages when
/// deserialization fails.
///
/// # Type Parameters
///
/// * `I` - Input type that implements [`serde::de::DeserializeOwned`]
///
/// # Example
///
/// ```
/// use rusty_claw::mcp_server::{TypedToolHandler, ToolHandler, ToolResult};
/// use serde::Deserialize;
/// use serde_json::json;
///
/// #[derive(Deserialize)]
/// struct AddInput {
///     a: f64,
///     b: f64,
/// }
///
/// # fn make_handler() -> impl rusty_claw::mcp_server::ToolHandler {
/// let handler = TypedToolHandler::new(|input: AddInput| async move {
///     Ok(ToolResult::text(format!("Result: {}", input.a + input.b)))
/// });
/// # handler
/// # }
/// ```
pub struct TypedToolHandler<I, F, Fut>
where
    I: DeserializeOwned + Send + Sync + 'static,
    F: Fn(I) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<ToolResult, ClawError>> + Send + 'static,
{
    handler: F,
    _phantom: std::marker::PhantomData<I>,
}

impl<I, F, Fut> TypedToolHandler<I, F, Fut>
where
    I: DeserializeOwned + Send + Sync + 'static,
    F: Fn(I) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<ToolResult, ClawError>> + Send + 'static,
{
    /// Create a new typed tool handler from an async closure
    ///
    /// The closure receives a fully-typed `I` value, automatically deserialized
    /// from the incoming `serde_json::Value` args. If deserialization fails, a
    /// descriptive [`ClawError::ToolExecution`] is returned that includes the
    /// target type name and the raw JSON.
    ///
    /// # Arguments
    ///
    /// * `handler` - Async closure that accepts a typed input `I`
    ///
    /// # Example
    ///
    /// ```
    /// use rusty_claw::mcp_server::{TypedToolHandler, ToolResult};
    /// use serde::Deserialize;
    ///
    /// #[derive(Deserialize)]
    /// struct MyInput {
    ///     name: String,
    /// }
    ///
    /// # fn make_handler() -> impl rusty_claw::mcp_server::ToolHandler {
    /// let handler = TypedToolHandler::new(|input: MyInput| async move {
    ///     Ok(ToolResult::text(format!("Hello, {}!", input.name)))
    /// });
    /// # handler
    /// # }
    /// ```
    pub fn new(handler: F) -> Self {
        Self {
            handler,
            _phantom: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<I, F, Fut> ToolHandler for TypedToolHandler<I, F, Fut>
where
    I: DeserializeOwned + Send + Sync + 'static,
    F: Fn(I) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<ToolResult, ClawError>> + Send + 'static,
{
    async fn call(&self, args: Value) -> Result<ToolResult, ClawError> {
        let typed_input: I = serde_json::from_value(args.clone()).map_err(|e| {
            ClawError::ToolExecution(format!(
                "Failed to deserialize tool args into {}: {}. Raw args: {}",
                std::any::type_name::<I>(),
                e,
                args
            ))
        })?;
        (self.handler)(typed_input).await
    }
}

/// MCP tool wrapper with metadata and handler
///
/// Represents a single tool that can be invoked via MCP.
/// Contains tool metadata (name, description, schema) and a reference
/// to the handler that executes the tool logic.
///
/// # Example
///
/// ```
/// use rusty_claw::prelude::*;
/// use rusty_claw::mcp_server::{SdkMcpTool, ToolHandler, ToolResult};
/// use async_trait::async_trait;
/// use serde_json::{json, Value};
/// use std::sync::Arc;
///
/// struct MyHandler;
///
/// #[async_trait]
/// impl ToolHandler for MyHandler {
///     async fn call(&self, _args: Value) -> Result<ToolResult, ClawError> {
///         Ok(ToolResult::text("Done"))
///     }
/// }
///
/// let tool = SdkMcpTool::new(
///     "my_tool",
///     "Does something useful",
///     json!({ "type": "object" }),
///     Arc::new(MyHandler),
/// );
/// ```

#[derive(Clone)]
pub struct SdkMcpTool {
    /// Tool name (must be unique within server)
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// JSON Schema for input validation
    pub input_schema: Value,
    /// Handler implementation
    handler: Arc<dyn ToolHandler>,
}

impl SdkMcpTool {
    /// Create a new MCP tool
    ///
    /// # Arguments
    ///
    /// * `name` - Tool name (must be unique within server)
    /// * `description` - Human-readable description
    /// * `input_schema` - JSON Schema for input validation
    /// * `handler` - Implementation of the tool logic
    ///
    /// # Example
    ///
    /// ```
    /// use rusty_claw::prelude::*;
    /// use rusty_claw::mcp_server::{SdkMcpTool, ToolHandler, ToolResult};
    /// use async_trait::async_trait;
    /// use serde_json::{json, Value};
    /// use std::sync::Arc;
    ///
    /// struct MyHandler;
    ///
    /// #[async_trait]
    /// impl ToolHandler for MyHandler {
    ///     async fn call(&self, _args: Value) -> Result<ToolResult, ClawError> {
    ///         Ok(ToolResult::text("Done"))
    ///     }
    /// }
    ///
    /// let tool = SdkMcpTool::new(
    ///     "my_tool",
    ///     "Does something",
    ///     json!({"type": "object"}),
    ///     Arc::new(MyHandler),
    /// );
    /// ```
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: Value,
        handler: Arc<dyn ToolHandler>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
            handler,
        }
    }

    /// Convert to MCP tool definition format
    ///
    /// Returns a JSON object suitable for the `tools/list` response.
    ///
    /// # Example
    ///
    /// ```
    /// use rusty_claw::prelude::*;
    /// use rusty_claw::mcp_server::{SdkMcpTool, ToolHandler, ToolResult};
    /// use async_trait::async_trait;
    /// use serde_json::{json, Value};
    /// use std::sync::Arc;
    ///
    /// struct MyHandler;
    ///
    /// #[async_trait]
    /// impl ToolHandler for MyHandler {
    ///     async fn call(&self, _args: Value) -> Result<ToolResult, ClawError> {
    ///         Ok(ToolResult::text("Done"))
    ///     }
    /// }
    ///
    /// let tool = SdkMcpTool::new("my_tool", "Does something", json!({"type": "object"}), Arc::new(MyHandler));
    /// let definition = tool.to_tool_definition();
    /// assert_eq!(definition["name"], "my_tool");
    /// ```
    pub fn to_tool_definition(&self) -> Value {
        json!({
            "name": self.name,
            "description": self.description,
            "inputSchema": self.input_schema,
        })
    }

    /// Execute the tool with the given arguments
    ///
    /// Delegates to the handler's `call` method.
    ///
    /// # Arguments
    ///
    /// * `args` - JSON object containing tool arguments
    ///
    /// # Returns
    ///
    /// * `Ok(ToolResult)` - Tool output
    /// * `Err(ClawError)` - Execution error
    ///
    /// # Example
    ///
    /// ```
    /// use rusty_claw::prelude::*;
    /// use rusty_claw::mcp_server::{SdkMcpTool, ToolHandler, ToolResult};
    /// use async_trait::async_trait;
    /// use serde_json::{json, Value};
    /// use std::sync::Arc;
    ///
    /// struct MyHandler;
    ///
    /// #[async_trait]
    /// impl ToolHandler for MyHandler {
    ///     async fn call(&self, args: Value) -> Result<ToolResult, ClawError> {
    ///         Ok(ToolResult::text(format!("Got: {}", args["x"])))
    ///     }
    /// }
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), ClawError> {
    /// let tool = SdkMcpTool::new("my_tool", "Does something", json!({"type": "object"}), Arc::new(MyHandler));
    /// let result = tool.execute(json!({"x": 42})).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn execute(&self, args: Value) -> Result<ToolResult, ClawError> {
        self.handler.call(args).await
    }
}

/// MCP server implementation with tool registry and JSON-RPC routing
///
/// Manages a collection of MCP tools and handles JSON-RPC messages
/// from the Claude CLI. Supports the following MCP methods:
/// - `initialize` - Returns server info and capabilities
/// - `tools/list` - Returns list of available tools
/// - `tools/call` - Executes a tool by name
///
/// # Example
///
/// ```
/// use rusty_claw::mcp_server::{SdkMcpServerImpl, SdkMcpTool, ToolHandler, ToolResult};
/// use rusty_claw::prelude::*;
/// use async_trait::async_trait;
/// use serde_json::{json, Value};
/// use std::sync::Arc;
///
/// struct MyHandler;
///
/// #[async_trait]
/// impl ToolHandler for MyHandler {
///     async fn call(&self, _args: Value) -> Result<ToolResult, ClawError> {
///         Ok(ToolResult::text("Done"))
///     }
/// }
///
/// let mut server = SdkMcpServerImpl::new("my_server", "1.0.0");
/// let tool = SdkMcpTool::new("my_tool", "Does something", json!({"type": "object"}), Arc::new(MyHandler));
/// server.register_tool(tool);
/// ```
pub struct SdkMcpServerImpl {
    /// Server name
    pub name: String,
    /// Server version
    pub version: String,
    /// Registered tools (keyed by name)
    tools: HashMap<String, SdkMcpTool>,
}

impl SdkMcpServerImpl {
    /// Create a new MCP server
    ///
    /// # Arguments
    ///
    /// * `name` - Server name (must match name in ClaudeAgentOptions)
    /// * `version` - Server version string
    ///
    /// # Example
    ///
    /// ```
    /// use rusty_claw::mcp_server::SdkMcpServerImpl;
    ///
    /// let server = SdkMcpServerImpl::new("my_server", "1.0.0");
    /// ```
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            tools: HashMap::new(),
        }
    }

    /// Create a new MCP server pre-populated with tools
    ///
    /// Convenience constructor that combines [`SdkMcpServerImpl::new`] and
    /// [`SdkMcpServerImpl::register_tool`] calls. Useful when you have a
    /// complete list of tools available upfront.
    ///
    /// # Arguments
    ///
    /// * `name` - Server name (must match name in `ClaudeAgentOptions`)
    /// * `version` - Server version string
    /// * `tools` - Tools to register immediately
    ///
    /// # Example
    ///
    /// ```
    /// use rusty_claw::mcp_server::{SdkMcpServerImpl, SdkMcpTool, ToolHandler, ToolResult};
    /// use rusty_claw::prelude::*;
    /// use async_trait::async_trait;
    /// use serde_json::{json, Value};
    /// use std::sync::Arc;
    ///
    /// struct MyHandler;
    ///
    /// #[async_trait]
    /// impl ToolHandler for MyHandler {
    ///     async fn call(&self, _args: Value) -> Result<ToolResult, ClawError> {
    ///         Ok(ToolResult::text("Done"))
    ///     }
    /// }
    ///
    /// let tools = vec![
    ///     SdkMcpTool::new("tool1", "Does X", json!({"type": "object"}), Arc::new(MyHandler)),
    ///     SdkMcpTool::new("tool2", "Does Y", json!({"type": "object"}), Arc::new(MyHandler)),
    /// ];
    /// let server = SdkMcpServerImpl::from_tools("my_server", "1.0.0", tools);
    /// assert_eq!(server.list_tools().len(), 2);
    /// ```
    pub fn from_tools(
        name: impl Into<String>,
        version: impl Into<String>,
        tools: Vec<SdkMcpTool>,
    ) -> Self {
        let mut server = Self::new(name, version);
        for tool in tools {
            server.register_tool(tool);
        }
        server
    }

        /// Register a tool with this server
    ///
    /// # Arguments
    ///
    /// * `tool` - Tool to register
    ///
    /// # Example
    ///
    /// ```
    /// use rusty_claw::mcp_server::{SdkMcpServerImpl, SdkMcpTool, ToolHandler, ToolResult};
    /// use rusty_claw::prelude::*;
    /// use async_trait::async_trait;
    /// use serde_json::{json, Value};
    /// use std::sync::Arc;
    ///
    /// struct MyHandler;
    ///
    /// #[async_trait]
    /// impl ToolHandler for MyHandler {
    ///     async fn call(&self, _args: Value) -> Result<ToolResult, ClawError> {
    ///         Ok(ToolResult::text("Done"))
    ///     }
    /// }
    ///
    /// let mut server = SdkMcpServerImpl::new("my_server", "1.0.0");
    /// let tool = SdkMcpTool::new("my_tool", "Does something", json!({"type": "object"}), Arc::new(MyHandler));
    /// server.register_tool(tool);
    /// ```
    pub fn register_tool(&mut self, tool: SdkMcpTool) {
        self.tools.insert(tool.name.clone(), tool);
    }

    /// Get a tool by name
    ///
    /// # Arguments
    ///
    /// * `name` - Tool name
    ///
    /// # Returns
    ///
    /// * `Some(&SdkMcpTool)` - Tool reference
    /// * `None` - Tool not found
    pub fn get_tool(&self, name: &str) -> Option<&SdkMcpTool> {
        self.tools.get(name)
    }

    /// List all registered tools
    ///
    /// Returns a vector of tool definitions suitable for the `tools/list` response.
    ///
    /// # Example
    ///
    /// ```
    /// use rusty_claw::mcp_server::{SdkMcpServerImpl, SdkMcpTool, ToolHandler, ToolResult};
    /// use rusty_claw::prelude::*;
    /// use async_trait::async_trait;
    /// use serde_json::{json, Value};
    /// use std::sync::Arc;
    ///
    /// struct MyHandler;
    ///
    /// #[async_trait]
    /// impl ToolHandler for MyHandler {
    ///     async fn call(&self, _args: Value) -> Result<ToolResult, ClawError> {
    ///         Ok(ToolResult::text("Done"))
    ///     }
    /// }
    ///
    /// let mut server = SdkMcpServerImpl::new("my_server", "1.0.0");
    /// server.register_tool(SdkMcpTool::new("tool1", "Does X", json!({"type": "object"}), Arc::new(MyHandler)));
    /// server.register_tool(SdkMcpTool::new("tool2", "Does Y", json!({"type": "object"}), Arc::new(MyHandler)));
    ///
    /// let tools = server.list_tools();
    /// assert_eq!(tools.len(), 2);
    /// ```
    pub fn list_tools(&self) -> Vec<Value> {
        self.tools
            .values()
            .map(|t| t.to_tool_definition())
            .collect()
    }

    /// Handle a JSON-RPC request
    ///
    /// Routes the request to the appropriate handler method based on the `method` field.
    /// Supports `initialize`, `tools/list`, and `tools/call`.
    ///
    /// # Arguments
    ///
    /// * `request` - JSON-RPC request object
    ///
    /// # Returns
    ///
    /// * `Ok(Value)` - JSON-RPC response
    /// * `Err(ClawError)` - Execution error
    ///
    /// # Example
    ///
    /// ```
    /// use rusty_claw::mcp_server::SdkMcpServerImpl;
    /// use serde_json::json;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let server = SdkMcpServerImpl::new("my_server", "1.0.0");
    /// let request = json!({
    ///     "jsonrpc": "2.0",
    ///     "id": 1,
    ///     "method": "initialize"
    /// });
    /// let response = server.handle_jsonrpc(request).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn handle_jsonrpc(&self, request: Value) -> Result<Value, ClawError> {
        let method = request["method"]
            .as_str()
            .ok_or_else(|| ClawError::ControlError("Missing method field".to_string()))?;

        match method {
            "initialize" => self.handle_initialize(&request),
            "notifications/initialized" => Ok(json_rpc_success(request["id"].clone(), json!({}))),
            "tools/list" => self.handle_tools_list(&request),
            "tools/call" => self.handle_tools_call(&request).await,
            _ => Ok(json_rpc_error(
                request["id"].clone(),
                -32601,
                format!("Method not found: {}", method),
            )),
        }
    }

    /// Handle `initialize` JSON-RPC request
    ///
    /// Returns server information and capabilities.
    fn handle_initialize(&self, request: &Value) -> Result<Value, ClawError> {
        Ok(json_rpc_success(
            request["id"].clone(),
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": self.name,
                    "version": self.version
                }
            }),
        ))
    }

    /// Handle `tools/list` JSON-RPC request
    ///
    /// Returns list of available tools.
    fn handle_tools_list(&self, request: &Value) -> Result<Value, ClawError> {
        Ok(json_rpc_success(
            request["id"].clone(),
            json!({
                "tools": self.list_tools()
            }),
        ))
    }

    /// Handle `tools/call` JSON-RPC request
    ///
    /// Executes the specified tool and returns the result.
    async fn handle_tools_call(&self, request: &Value) -> Result<Value, ClawError> {
        let params = request["params"]
            .as_object()
            .ok_or_else(|| ClawError::ControlError("Missing params".to_string()))?;

        let name = params["name"]
            .as_str()
            .ok_or_else(|| ClawError::ControlError("Missing tool name".to_string()))?;

        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));

        // Find tool
        let tool = match self.get_tool(name) {
            Some(t) => t,
            None => {
                return Ok(json_rpc_error(
                    request["id"].clone(),
                    -32602,
                    format!("Tool not found: {}", name),
                ))
            }
        };

        // Execute tool
        // Tool execution failures are MCP application-level errors (isError: true),
        // not JSON-RPC protocol errors. Using -32603 would tell the caller that
        // the *protocol* failed, not the tool itself.
        match tool.execute(arguments).await {
            Ok(result) => Ok(json_rpc_success(request["id"].clone(), result)),
            Err(e) => Ok(json_rpc_success(
                request["id"].clone(),
                ToolResult::error(format!("Tool execution failed: {}", e)),
            )),
        }
    }
}

/// Create an SDK MCP server from a name, version, and list of tools
///
/// This is the primary convenience function for creating an SDK-hosted MCP server.
/// It replaces the manual sequence of:
/// 1. `SdkMcpServerImpl::new()`
/// 2. `SdkMcpTool::new()` for each tool
/// 3. `server.register_tool()` for each tool
/// 4. `Arc::new(server)`
///
/// # Arguments
///
/// * `name` - Server name (must match the entry in `ClaudeAgentOptions` `mcp_servers`)
/// * `version` - Server version string
/// * `tools` - Tools to register with the server
///
/// # Returns
///
/// An `Arc<SdkMcpServerImpl>` ready to pass to
/// `ClaudeClient::register_mcp_message_handler`.
///
/// # Example
///
/// ```
/// use rusty_claw::mcp_server::{create_sdk_mcp_server, SdkMcpTool, ToolHandler, ToolResult};
/// use rusty_claw::prelude::*;
/// use async_trait::async_trait;
/// use serde_json::{json, Value};
/// use std::sync::Arc;
///
/// struct EchoHandler;
///
/// #[async_trait]
/// impl ToolHandler for EchoHandler {
///     async fn call(&self, args: Value) -> Result<ToolResult, ClawError> {
///         let msg = args["message"].as_str().unwrap_or("(none)");
///         Ok(ToolResult::text(format!("Echo: {}", msg)))
///     }
/// }
///
/// let server = create_sdk_mcp_server(
///     "my_tools",
///     "1.0.0",
///     vec![
///         SdkMcpTool::new(
///             "echo",
///             "Echo a message",
///             json!({"type": "object", "properties": {"message": {"type": "string"}}}),
///             Arc::new(EchoHandler),
///         ),
///     ],
/// );
/// // server is Arc<SdkMcpServerImpl>, ready for register_mcp_message_handler()
/// ```
pub fn create_sdk_mcp_server(
    name: impl Into<String>,
    version: impl Into<String>,
    tools: Vec<SdkMcpTool>,
) -> Arc<SdkMcpServerImpl> {
    Arc::new(SdkMcpServerImpl::from_tools(name, version, tools))
}

/// Registry for multiple MCP servers
///
/// Implements [`McpMessageHandler`] to route JSON-RPC messages from the CLI
/// to the appropriate SDK-hosted MCP server.
///
/// # Example
///
/// ```
/// use rusty_claw::mcp_server::{SdkMcpServerRegistry, SdkMcpServerImpl};
/// use rusty_claw::control::handlers::McpMessageHandler;
/// use serde_json::json;
/// use std::sync::Arc;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut registry = SdkMcpServerRegistry::new();
/// let server = SdkMcpServerImpl::new("my_server", "1.0.0");
/// registry.register(server);
///
/// let message = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize"});
/// let response = registry.handle("my_server", message).await?;
/// # Ok(())
/// # }
/// ```
pub struct SdkMcpServerRegistry {
    /// Servers keyed by name
    servers: HashMap<String, SdkMcpServerImpl>,
}

impl SdkMcpServerRegistry {
    /// Create a new empty registry
    ///
    /// # Example
    ///
    /// ```
    /// use rusty_claw::mcp_server::SdkMcpServerRegistry;
    ///
    /// let registry = SdkMcpServerRegistry::new();
    /// ```
    pub fn new() -> Self {
        Self {
            servers: HashMap::new(),
        }
    }

    /// Register an MCP server
    ///
    /// # Arguments
    ///
    /// * `server` - Server to register
    ///
    /// # Example
    ///
    /// ```
    /// use rusty_claw::mcp_server::{SdkMcpServerRegistry, SdkMcpServerImpl};
    ///
    /// let mut registry = SdkMcpServerRegistry::new();
    /// let server = SdkMcpServerImpl::new("my_server", "1.0.0");
    /// registry.register(server);
    /// ```
    pub fn register(&mut self, server: SdkMcpServerImpl) {
        self.servers.insert(server.name.clone(), server);
    }

    /// Get a server by name
    ///
    /// # Arguments
    ///
    /// * `name` - Server name
    ///
    /// # Returns
    ///
    /// * `Some(&SdkMcpServerImpl)` - Server reference
    /// * `None` - Server not found
    pub fn get(&self, name: &str) -> Option<&SdkMcpServerImpl> {
        self.servers.get(name)
    }
}

impl Default for SdkMcpServerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl McpMessageHandler for SdkMcpServerRegistry {
    async fn handle(&self, server_name: &str, message: Value) -> Result<Value, ClawError> {
        let server = self
            .get(server_name)
            .ok_or_else(|| ClawError::ControlError(format!("Server not found: {}", server_name)))?;

        server.handle_jsonrpc(message).await
    }
}

/// Create a JSON-RPC success response
///
/// # Arguments
///
/// * `id` - Request ID
/// * `result` - Result data
///
/// # Returns
///
/// JSON-RPC 2.0 success response
fn json_rpc_success(id: Value, result: impl Serialize) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

/// Create a JSON-RPC error response
///
/// # Arguments
///
/// * `id` - Request ID
/// * `code` - Error code
/// * `message` - Error message
///
/// # Returns
///
/// JSON-RPC 2.0 error response
fn json_rpc_error(id: Value, code: i32, message: String) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockHandler {
        response: String,
    }

    #[async_trait]
    impl ToolHandler for MockHandler {
        async fn call(&self, _args: Value) -> Result<ToolResult, ClawError> {
            Ok(ToolResult::text(&self.response))
        }
    }

    struct ErrorHandler;

    #[async_trait]
    impl ToolHandler for ErrorHandler {
        async fn call(&self, _args: Value) -> Result<ToolResult, ClawError> {
            Err(ClawError::ControlError("Handler error".to_string()))
        }
    }

    #[test]
    fn test_tool_content_text() {
        let content = ToolContent::text("Hello");
        match content {
            ToolContent::Text { text } => assert_eq!(text, "Hello"),
            _ => panic!("Expected Text variant"),
        }
    }

    #[test]
    fn test_tool_content_image() {
        let content = ToolContent::image("data123", "image/png");
        match content {
            ToolContent::Image { data, mime_type } => {
                assert_eq!(data, "data123");
                assert_eq!(mime_type, "image/png");
            }
            _ => panic!("Expected Image variant"),
        }
    }

    #[test]
    fn test_tool_result_text() {
        let result = ToolResult::text("Success");
        assert_eq!(result.content.len(), 1);
        assert!(result.is_error.is_none());
    }

    #[test]
    fn test_tool_result_error() {
        let result = ToolResult::error("Failed");
        assert_eq!(result.content.len(), 1);
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn test_tool_result_new() {
        let result = ToolResult::new(vec![
            ToolContent::text("Text"),
            ToolContent::image("data", "image/png"),
        ]);
        assert_eq!(result.content.len(), 2);
        assert!(result.is_error.is_none());
    }

    #[tokio::test]
    async fn test_tool_handler() {
        let handler = MockHandler {
            response: "Test".to_string(),
        };
        let result = handler.call(json!({})).await.unwrap();
        match &result.content[0] {
            ToolContent::Text { text } => assert_eq!(text, "Test"),
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn test_sdk_mcp_tool_new() {
        let handler = Arc::new(MockHandler {
            response: "Test".to_string(),
        });
        let tool = SdkMcpTool::new(
            "test_tool",
            "Test description",
            json!({"type": "object"}),
            handler,
        );
        assert_eq!(tool.name, "test_tool");
        assert_eq!(tool.description, "Test description");
    }

    #[test]
    fn test_sdk_mcp_tool_to_definition() {
        let handler = Arc::new(MockHandler {
            response: "Test".to_string(),
        });
        let tool = SdkMcpTool::new(
            "test_tool",
            "Test description",
            json!({"type": "object"}),
            handler,
        );
        let def = tool.to_tool_definition();
        assert_eq!(def["name"], "test_tool");
        assert_eq!(def["description"], "Test description");
        assert_eq!(def["inputSchema"]["type"], "object");
    }

    #[tokio::test]
    async fn test_sdk_mcp_tool_execute() {
        let handler = Arc::new(MockHandler {
            response: "Executed".to_string(),
        });
        let tool = SdkMcpTool::new("test_tool", "Test", json!({"type": "object"}), handler);
        let result = tool.execute(json!({})).await.unwrap();
        match &result.content[0] {
            ToolContent::Text { text } => assert_eq!(text, "Executed"),
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn test_sdk_mcp_server_new() {
        let server = SdkMcpServerImpl::new("test_server", "1.0.0");
        assert_eq!(server.name, "test_server");
        assert_eq!(server.version, "1.0.0");
        assert_eq!(server.tools.len(), 0);
    }

    #[test]
    fn test_sdk_mcp_server_register_tool() {
        let mut server = SdkMcpServerImpl::new("test_server", "1.0.0");
        let handler = Arc::new(MockHandler {
            response: "Test".to_string(),
        });
        let tool = SdkMcpTool::new("tool1", "Test", json!({"type": "object"}), handler);
        server.register_tool(tool);
        assert_eq!(server.tools.len(), 1);
        assert!(server.get_tool("tool1").is_some());
    }

    #[test]
    fn test_sdk_mcp_server_list_tools() {
        let mut server = SdkMcpServerImpl::new("test_server", "1.0.0");
        let handler = Arc::new(MockHandler {
            response: "Test".to_string(),
        });
        server.register_tool(SdkMcpTool::new(
            "tool1",
            "Test 1",
            json!({"type": "object"}),
            handler.clone(),
        ));
        server.register_tool(SdkMcpTool::new(
            "tool2",
            "Test 2",
            json!({"type": "object"}),
            handler,
        ));
        let tools = server.list_tools();
        assert_eq!(tools.len(), 2);
    }

    #[tokio::test]
    async fn test_handle_initialize() {
        let server = SdkMcpServerImpl::new("test_server", "1.0.0");
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize"
        });
        let response = server.handle_jsonrpc(request).await.unwrap();
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 1);
        assert_eq!(response["result"]["serverInfo"]["name"], "test_server");
    }

    #[tokio::test]
    async fn test_handle_tools_list() {
        let mut server = SdkMcpServerImpl::new("test_server", "1.0.0");
        let handler = Arc::new(MockHandler {
            response: "Test".to_string(),
        });
        server.register_tool(SdkMcpTool::new(
            "tool1",
            "Test",
            json!({"type": "object"}),
            handler,
        ));

        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list"
        });
        let response = server.handle_jsonrpc(request).await.unwrap();
        assert_eq!(response["result"]["tools"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_handle_tools_call() {
        let mut server = SdkMcpServerImpl::new("test_server", "1.0.0");
        let handler = Arc::new(MockHandler {
            response: "Result".to_string(),
        });
        server.register_tool(SdkMcpTool::new(
            "tool1",
            "Test",
            json!({"type": "object"}),
            handler,
        ));

        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "tool1",
                "arguments": {}
            }
        });
        let response = server.handle_jsonrpc(request).await.unwrap();
        assert_eq!(response["result"]["content"][0]["text"], "Result");
    }

    #[tokio::test]
    async fn test_handle_tools_call_not_found() {
        let server = SdkMcpServerImpl::new("test_server", "1.0.0");
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "nonexistent",
                "arguments": {}
            }
        });
        let response = server.handle_jsonrpc(request).await.unwrap();
        assert!(response["error"].is_object());
        assert_eq!(response["error"]["code"], -32602);
    }

    #[tokio::test]
    async fn test_handle_tools_call_handler_error() {
        let mut server = SdkMcpServerImpl::new("test_server", "1.0.0");
        server.register_tool(SdkMcpTool::new(
            "error_tool",
            "Test",
            json!({"type": "object"}),
            Arc::new(ErrorHandler),
        ));

        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "error_tool",
                "arguments": {}
            }
        });
        let response = server.handle_jsonrpc(request).await.unwrap();
        // Tool execution failures are MCP application-level errors (isError: true in result),
        // not JSON-RPC protocol errors. The response must be a success response at the
        // protocol level so the caller can read the error content.
        assert!(response["error"].is_null());
        assert!(response["result"].is_object());
        assert_eq!(response["result"]["isError"], true);
    }

    #[tokio::test]
    async fn test_handle_unknown_method() {
        let server = SdkMcpServerImpl::new("test_server", "1.0.0");
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "unknown/method"
        });
        let response = server.handle_jsonrpc(request).await.unwrap();
        assert!(response["error"].is_object());
        assert_eq!(response["error"]["code"], -32601);
    }

    #[test]
    fn test_registry_new() {
        let registry = SdkMcpServerRegistry::new();
        assert_eq!(registry.servers.len(), 0);
    }

    #[test]
    fn test_registry_register() {
        let mut registry = SdkMcpServerRegistry::new();
        let server = SdkMcpServerImpl::new("test_server", "1.0.0");
        registry.register(server);
        assert_eq!(registry.servers.len(), 1);
        assert!(registry.get("test_server").is_some());
    }

    #[tokio::test]
    async fn test_registry_handle() {
        let mut registry = SdkMcpServerRegistry::new();
        let server = SdkMcpServerImpl::new("test_server", "1.0.0");
        registry.register(server);

        let message = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize"
        });
        let response = registry.handle("test_server", message).await.unwrap();
        assert_eq!(response["result"]["serverInfo"]["name"], "test_server");
    }

    #[tokio::test]
    async fn test_registry_handle_server_not_found() {
        let registry = SdkMcpServerRegistry::new();
        let message = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize"
        });
        let result = registry.handle("nonexistent", message).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_json_rpc_success() {
        let response = json_rpc_success(json!(1), json!({"status": "ok"}));
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 1);
        assert_eq!(response["result"]["status"], "ok");
    }

    #[test]
    fn test_json_rpc_error() {
        let response = json_rpc_error(json!(1), -32601, "Method not found".to_string());
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 1);
        assert_eq!(response["error"]["code"], -32601);
        assert_eq!(response["error"]["message"], "Method not found");
    }

    // Tests for TypedToolHandler (6c4)

    #[tokio::test]
    async fn test_typed_tool_handler_success() {
        use serde::Deserialize;

        #[derive(Deserialize)]
        struct AddInput {
            a: f64,
            b: f64,
        }

        let handler = TypedToolHandler::new(|input: AddInput| async move {
            Ok(ToolResult::text(format!("{}", input.a + input.b)))
        });

        let result = handler.call(json!({"a": 3.0, "b": 4.0})).await.unwrap();
        match &result.content[0] {
            ToolContent::Text { text } => assert_eq!(text, "7"),
            _ => panic!("Expected text"),
        }
    }

    #[tokio::test]
    async fn test_typed_tool_handler_deserialization_error() {
        use serde::Deserialize;

        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct StrictInput {
            count: u32,
        }

        let handler = TypedToolHandler::new(|_input: StrictInput| async move {
            Ok(ToolResult::text("ok"))
        });

        // "count" is missing → deserialization error
        let result = handler.call(json!({"wrong_field": 1})).await;
        assert!(result.is_err(), "Expected error for bad input");
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Failed to deserialize tool args"), "Error should mention deserialization: {}", err);
    }

    #[tokio::test]
    async fn test_typed_tool_handler_optional_field() {
        use serde::Deserialize;

        #[derive(Deserialize)]
        struct Input {
            required: String,
            optional: Option<String>,
        }

        let handler = TypedToolHandler::new(|input: Input| async move {
            let suffix = input.optional.unwrap_or_else(|| "none".to_string());
            Ok(ToolResult::text(format!("{}:{}", input.required, suffix)))
        });

        // With optional field
        let r1 = handler.call(json!({"required": "hello", "optional": "world"})).await.unwrap();
        match &r1.content[0] {
            ToolContent::Text { text } => assert_eq!(text, "hello:world"),
            _ => panic!(),
        }

        // Without optional field
        let r2 = handler.call(json!({"required": "hello"})).await.unwrap();
        match &r2.content[0] {
            ToolContent::Text { text } => assert_eq!(text, "hello:none"),
            _ => panic!(),
        }
    }

    // Tests for create_sdk_mcp_server and from_tools (abh)

    #[test]
    fn test_sdk_mcp_server_from_tools() {
        let handler = Arc::new(MockHandler {
            response: "ok".to_string(),
        });
        let tools = vec![
            SdkMcpTool::new("t1", "Tool 1", json!({"type": "object"}), handler.clone()),
            SdkMcpTool::new("t2", "Tool 2", json!({"type": "object"}), handler),
        ];
        let server = SdkMcpServerImpl::from_tools("my_server", "2.0.0", tools);
        assert_eq!(server.name, "my_server");
        assert_eq!(server.version, "2.0.0");
        assert_eq!(server.list_tools().len(), 2);
        assert!(server.get_tool("t1").is_some());
        assert!(server.get_tool("t2").is_some());
    }

    #[test]
    fn test_create_sdk_mcp_server() {
        let handler = Arc::new(MockHandler {
            response: "result".to_string(),
        });
        let server = create_sdk_mcp_server(
            "test_server",
            "1.0.0",
            vec![SdkMcpTool::new(
                "my_tool",
                "Test",
                json!({"type": "object"}),
                handler,
            )],
        );
        assert_eq!(server.name, "test_server");
        assert_eq!(server.list_tools().len(), 1);
    }

    #[test]
    fn test_create_sdk_mcp_server_empty() {
        let server = create_sdk_mcp_server("empty_server", "0.1.0", vec![]);
        assert_eq!(server.name, "empty_server");
        assert_eq!(server.list_tools().len(), 0);
    }

    #[tokio::test]
    async fn test_create_sdk_mcp_server_can_handle_requests() {
        let handler = Arc::new(MockHandler {
            response: "hello".to_string(),
        });
        let server = create_sdk_mcp_server(
            "hello_server",
            "1.0.0",
            vec![SdkMcpTool::new(
                "greet",
                "Greet",
                json!({"type": "object"}),
                handler,
            )],
        );

        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {"name": "greet", "arguments": {}}
        });
        let response = server.handle_jsonrpc(request).await.unwrap();
        assert_eq!(response["result"]["content"][0]["text"], "hello");
    }
}

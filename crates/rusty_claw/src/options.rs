//! Configuration options and builder pattern for Claude agent sessions
//!
//! This module provides `ClaudeAgentOptions` for configuring Claude agent behavior,
//! including prompt settings, tools, permissions, session management, and environment.
//!
//! # Example
//!
//! ```
//! use rusty_claw::options::{ClaudeAgentOptions, PermissionMode, SystemPrompt};
//!
//! let options = ClaudeAgentOptions::builder()
//!     .allowed_tools(vec!["Read".to_string(), "Bash".to_string()])
//!     .permission_mode(PermissionMode::AcceptEdits)
//!     .max_turns(5)
//!     .model("claude-sonnet-4")
//!     .build();
//! ```
//!
//! # Builder Pattern
//!
//! All fields have sensible defaults. Use the builder pattern for convenient configuration:
//!
//! ```
//! use rusty_claw::options::{ClaudeAgentOptions, SystemPrompt};
//!
//! let options = ClaudeAgentOptions::builder()
//!     .system_prompt(SystemPrompt::Custom("You are a helpful assistant".to_string()))
//!     .max_turns(10)
//!     .build();
//! ```

use crate::control::handlers::CanUseToolHandler;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Known beta feature identifiers for use with [`ClaudeAgentOptions::betas`]
///
/// These constants provide discoverable names for known Claude CLI beta features.
/// Using unknown beta strings also works — these constants exist for documentation
/// and discoverability, not for validation.
///
/// # Example
///
/// ```
/// use rusty_claw::options::{ClaudeAgentOptions, SdkBeta};
///
/// let options = ClaudeAgentOptions::builder()
///     .betas(vec![SdkBeta::INTERLEAVED_THINKING.to_string()])
///     .build();
/// ```
pub struct SdkBeta;

impl SdkBeta {
    /// Extended thinking interleaved with responses (2025-05-14)
    pub const INTERLEAVED_THINKING: &'static str = "interleaved-thinking-2025-05-14";
    /// Increased context window (1M tokens) (2025-08-07)
    pub const CONTEXT_1M: &'static str = "context-1m-2025-08-07";
    /// Increased max output tokens for claude-3-5-sonnet (2024-07-15)
    pub const MAX_TOKENS_65K: &'static str = "max-tokens-3-5-sonnet-2024-07-15";
    /// Computer use tools beta (2024-10-22)
    pub const COMPUTER_USE: &'static str = "computer-use-2024-10-22";
    /// Batch processing API (2024-09-24)
    pub const MESSAGE_BATCHES: &'static str = "message-batches-2024-09-24";
    /// Files API beta (2025-04-14)
    pub const FILES_API: &'static str = "files-api-2025-04-14";
}

/// System prompt configuration
#[derive(Debug, Clone)]
pub enum SystemPrompt {
    /// Custom system prompt text
    Custom(String),
    /// Named preset system prompt
    Preset {
        /// Preset name
        preset: String,
    },
}

/// Permission mode for tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    /// Default permission mode
    Default,
    /// Accept all edit operations
    AcceptEdits,
    /// Bypass all permission checks
    BypassPermissions,
    /// Plan mode requiring approval
    Plan,
    /// Allow all tool use without prompting
    Allow,
    /// Prompt user for each tool use
    Ask,
    /// Deny all tool use
    Deny,
    /// Use custom permission logic via hooks
    Custom,
}

impl PermissionMode {
    /// Convert to CLI argument format (camelCase, matching CLI's allowed choices)
    pub fn to_cli_arg(&self) -> &str {
        match self {
            PermissionMode::Default => "default",
            PermissionMode::AcceptEdits => "acceptEdits",
            PermissionMode::BypassPermissions => "bypassPermissions",
            PermissionMode::Plan => "plan",
            PermissionMode::Allow => "allow",
            PermissionMode::Ask => "ask",
            PermissionMode::Deny => "deny",
            PermissionMode::Custom => "custom",
        }
    }
}

// ============================================================================
// External MCP Server Configuration Types (bvo, 9be, xik)
// ============================================================================

/// Configuration for a stdio-based external MCP server.
///
/// A stdio server is a local child process that communicates via stdin/stdout
/// using the MCP JSON-RPC protocol. This is the most common MCP server type.
///
/// # Example
///
/// ```
/// use rusty_claw::options::McpStdioServerConfig;
/// use std::collections::HashMap;
///
/// let config = McpStdioServerConfig {
///     command: "npx".to_string(),
///     args: vec!["-y".to_string(), "@modelcontextprotocol/server-filesystem".to_string()],
///     env: HashMap::new(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpStdioServerConfig {
    /// Executable command to launch the MCP server process.
    pub command: String,
    /// Command-line arguments passed to the server process.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// Additional environment variables set for the server process.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,
}

/// Configuration for a Server-Sent Events (SSE) remote MCP server.
///
/// SSE servers run as HTTP services and stream responses via the SSE protocol.
/// They are common for managed/shared MCP deployments.
///
/// # Example
///
/// ```
/// use rusty_claw::options::McpSSEServerConfig;
/// use std::collections::HashMap;
///
/// let config = McpSSEServerConfig {
///     url: "https://mcp.example.com/sse".to_string(),
///     headers: HashMap::new(),
///     timeout: Some(30.0),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSSEServerConfig {
    /// URL of the SSE MCP server endpoint.
    pub url: String,
    /// HTTP headers to include with every request (e.g., authentication tokens).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,
    /// Connection timeout in seconds. `None` uses the CLI default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<f64>,
}

/// Configuration for an HTTP-based remote MCP server.
///
/// HTTP servers communicate via standard request/response HTTP calls.
/// They are suited for stateless MCP tool deployments.
///
/// # Example
///
/// ```
/// use rusty_claw::options::McpHttpServerConfig;
/// use std::collections::HashMap;
///
/// let config = McpHttpServerConfig {
///     url: "https://mcp.example.com/http".to_string(),
///     headers: HashMap::new(),
///     timeout: None,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpHttpServerConfig {
    /// URL of the HTTP MCP server endpoint.
    pub url: String,
    /// HTTP headers to include with every request (e.g., authentication tokens).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,
    /// Connection timeout in seconds. `None` uses the CLI default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<f64>,
}

/// Discriminated union of all supported external MCP server configurations.
///
/// Use the appropriate variant depending on how the MCP server is hosted:
/// - [`McpServerConfig::Stdio`] — local child process (most common)
/// - [`McpServerConfig::Sse`]   — remote SSE HTTP endpoint
/// - [`McpServerConfig::Http`]  — remote HTTP endpoint
///
/// # Example
///
/// ```
/// use rusty_claw::options::{McpServerConfig, McpStdioServerConfig};
/// use std::collections::HashMap;
///
/// let server = McpServerConfig::Stdio(McpStdioServerConfig {
///     command: "npx".to_string(),
///     args: vec!["-y".to_string(), "@modelcontextprotocol/server-filesystem".to_string()],
///     env: HashMap::new(),
/// });
///
/// // Serializes to: {"type":"stdio","command":"npx","args":["-y","..."]}
/// let json = serde_json::to_string(&server).unwrap();
/// assert!(json.contains("\"type\":\"stdio\""));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum McpServerConfig {
    /// Local stdio child-process MCP server.
    Stdio(McpStdioServerConfig),
    /// Remote SSE-based MCP server.
    Sse(McpSSEServerConfig),
    /// Remote HTTP-based MCP server.
    Http(McpHttpServerConfig),
}

/// SDK-managed MCP server configuration
///
/// This struct represents the minimal configuration needed to register
/// an SDK-hosted MCP server with the Claude CLI during initialization.
/// The actual server implementation is in [`crate::mcp_server::SdkMcpServerImpl`].
///
/// # Example
///
/// ```
/// use rusty_claw::options::SdkMcpServer;
///
/// let config = SdkMcpServer {
///     name: "my_server".to_string(),
///     version: "1.0.0".to_string(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkMcpServer {
    /// Server name (must be unique)
    pub name: String,
    /// Server version
    pub version: String,
}

/// Hook event type - triggers for lifecycle callbacks
///
/// # Examples
///
/// ```
/// use rusty_claw::prelude::*;
///
/// let event = HookEvent::PreToolUse;
/// assert_eq!(format!("{:?}", event), "PreToolUse");
/// ```
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum HookEvent {
    /// Before a tool is invoked
    PreToolUse,
    /// After a tool successfully completes
    PostToolUse,
    /// After a tool fails
    PostToolUseFailure,
    /// When user submits a prompt
    UserPromptSubmit,
    /// When session stops
    Stop,
    /// When a subagent stops
    SubagentStop,
    /// When a subagent starts
    SubagentStart,
    /// Before conversation compaction
    PreCompact,
    /// System notification event
    Notification,
    /// Permission request from Claude
    PermissionRequest,
}

/// Hook matcher for pattern-based hook triggering.
///
/// A `HookMatcher` can filter hooks by:
/// - Tool name pattern (exact match or glob-style wildcard with `*`)
/// - Event types (which lifecycle events this matcher responds to)
/// - Timeout (maximum milliseconds before the hook is cancelled)
///
/// # Examples
///
/// ```
/// use rusty_claw::prelude::*;
///
/// // Match all tools
/// let matcher = HookMatcher::all();
/// assert!(matcher.matches("Bash"));
/// assert!(matcher.matches("Read"));
///
/// // Match specific tool
/// let matcher = HookMatcher::tool("Bash");
/// assert!(matcher.matches("Bash"));
/// assert!(!matcher.matches("Read"));
///
/// // Wildcard matching: all MCP tools
/// let matcher = HookMatcher::tool("mcp__*");
/// assert!(matcher.matches("mcp__text_tools__word_count"));
/// assert!(!matcher.matches("Bash"));
///
/// // With timeout
/// let matcher = HookMatcher::all().with_timeout_ms(5000);
/// assert_eq!(matcher.timeout_ms, Some(5000));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookMatcher {
    /// Tool name pattern to match (e.g., "Bash", "mcp__*", or `None` for all).
    ///
    /// Supports glob-style wildcard patterns with `*`:
    /// - `"mcp__*"` matches any MCP tool
    /// - `"bash*"` matches any tool starting with "bash"
    /// - `"*_tool"` matches any tool ending with "_tool"
    /// - `"Bash"` matches only the exact tool name "Bash"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,

    /// Event types this matcher responds to.
    ///
    /// When non-empty, the hook only fires for the listed events.
    /// When empty, the hook fires for all event types (same as the previous behavior).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub hooks: Vec<HookEvent>,

    /// Per-hook timeout in milliseconds.
    ///
    /// If the hook handler takes longer than this to respond, it is cancelled
    /// and the default allow/deny decision is applied. This prevents slow
    /// external services from blocking the entire session.
    ///
    /// When `None`, no timeout is enforced.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

impl HookMatcher {
    /// Create a matcher that matches all tools with no timeout.
    pub fn all() -> Self {
        Self {
            tool_name: None,
            hooks: Vec::new(),
            timeout_ms: None,
        }
    }

    /// Create a matcher for a specific tool name or wildcard pattern.
    ///
    /// The pattern supports `*` as a wildcard. For example:
    /// - `"mcp__*"` matches all MCP tools
    /// - `"Bash"` matches only the Bash tool
    pub fn tool(name: impl Into<String>) -> Self {
        Self {
            tool_name: Some(name.into()),
            hooks: Vec::new(),
            timeout_ms: None,
        }
    }

    /// Set the event types this matcher responds to.
    ///
    /// When set, the hook fires only for the listed event types.
    pub fn with_events(mut self, events: Vec<HookEvent>) -> Self {
        self.hooks = events;
        self
    }

    /// Set the per-hook timeout in milliseconds.
    ///
    /// If the hook exceeds this duration, it is cancelled and the default
    /// decision is applied.
    pub fn with_timeout_ms(mut self, ms: u64) -> Self {
        self.timeout_ms = Some(ms);
        self
    }

    /// Check if this matcher matches the given tool name.
    ///
    /// Supports glob-style wildcard patterns with `*`:
    /// - A leading `*` matches any prefix
    /// - A trailing `*` matches any suffix
    /// - `*` alone matches everything (same as `None`)
    /// - Multiple `*` patterns are supported
    pub fn matches(&self, tool_name: &str) -> bool {
        match &self.tool_name {
            None => true, // Match all
            Some(pattern) => match_glob_pattern(pattern, tool_name),
        }
    }

    /// Check if this matcher responds to the given event type.
    ///
    /// Returns `true` if the `hooks` list is empty (matches all events)
    /// or if the given event is in the list.
    pub fn matches_event(&self, event: &HookEvent) -> bool {
        self.hooks.is_empty() || self.hooks.contains(event)
    }
}

/// Perform glob-style wildcard pattern matching.
///
/// Supports `*` as a wildcard that matches any sequence of characters.
/// For example: `"mcp__*"` matches `"mcp__text_tools__word_count"`.
fn match_glob_pattern(pattern: &str, value: &str) -> bool {
    // Split on '*' and match each segment in order
    let parts: Vec<&str> = pattern.split('*').collect();

    if parts.len() == 1 {
        // No wildcard: exact match
        return pattern == value;
    }

    let mut remaining = value;

    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            // First part: must be a prefix
            if !remaining.starts_with(part) {
                return false;
            }
            remaining = &remaining[part.len()..];
        } else if i == parts.len() - 1 {
            // Last part: must be a suffix
            if !remaining.ends_with(part) {
                return false;
            }
        } else {
            // Middle part: must appear somewhere in remaining
            match remaining.find(part) {
                Some(pos) => remaining = &remaining[pos + part.len()..],
                None => return false,
            }
        }
    }

    true
}

/// Agent definition for subagents (placeholder for future agent tasks)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDefinition {
    /// Agent description
    pub description: String,
    /// Agent prompt
    pub prompt: String,
    /// Allowed tools
    pub tools: Vec<String>,
    /// Model override
    pub model: Option<String>,
}

/// Sandbox settings for controlling the Claude agent's execution environment
///
/// When sandboxing is enabled, the agent runs in a restricted environment
/// that prevents irreversible filesystem changes and limits network access.
/// This is critical for safe agentic deployments in production.
///
/// # Example
///
/// ```
/// use rusty_claw::options::SandboxSettings;
///
/// let settings = SandboxSettings {
///     enabled: true,
///     auto_allow_bash_if_sandboxed: true,
///     excluded_commands: vec!["rm".to_string(), "sudo".to_string()],
///     allow_unsandboxed_commands: vec![],
///     network: Some(false),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxSettings {
    /// Whether sandboxing is enabled
    pub enabled: bool,
    /// Automatically allow Bash tool when running in sandbox
    pub auto_allow_bash_if_sandboxed: bool,
    /// Commands that are excluded from the sandbox (always blocked)
    pub excluded_commands: Vec<String>,
    /// Commands that are allowed to run without sandbox restrictions
    pub allow_unsandboxed_commands: Vec<String>,
    /// Network access setting: Some(true) = allow, Some(false) = deny, None = default
    pub network: Option<bool>,
}

impl SandboxSettings {
    /// Create sandbox settings with sandboxing enabled and safe defaults
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            auto_allow_bash_if_sandboxed: false,
            excluded_commands: vec![],
            allow_unsandboxed_commands: vec![],
            network: None,
        }
    }

    /// Create sandbox settings with sandboxing disabled
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            auto_allow_bash_if_sandboxed: false,
            excluded_commands: vec![],
            allow_unsandboxed_commands: vec![],
            network: None,
        }
    }
}

/// Configuration options for Claude agent sessions
///
/// This struct provides comprehensive configuration for Claude agent behavior,
/// including prompt settings, tool permissions, session management, and environment.
///
/// # Example
///
/// ```
/// use rusty_claw::options::ClaudeAgentOptions;
///
/// let options = ClaudeAgentOptions::builder()
///     .max_turns(5)
///     .model("claude-sonnet-4")
///     .build();
/// ```
#[derive(Default)]
pub struct ClaudeAgentOptions {
    // Prompt & behavior
    /// System prompt configuration
    pub system_prompt: Option<SystemPrompt>,
    /// Additional text to append to system prompt
    pub append_system_prompt: Option<String>,
    /// Maximum conversation turns
    pub max_turns: Option<u32>,
    /// Model identifier (e.g., "claude-sonnet-4")
    pub model: Option<String>,

    // Tools & permissions
    /// Tools to explicitly allow
    pub allowed_tools: Vec<String>,
    /// Tools to explicitly disallow
    pub disallowed_tools: Vec<String>,
    /// Permission mode for tool execution
    pub permission_mode: Option<PermissionMode>,
    /// Tools that require permission prompts (passed as `--permission-prompt-tool-name <tool>` for each)
    pub permission_prompt_tool_allowlist: Vec<String>,

    // MCP (placeholder for future tasks)
    /// MCP server configurations
    pub mcp_servers: HashMap<String, McpServerConfig>,
    /// SDK-managed MCP servers
    pub sdk_mcp_servers: Vec<SdkMcpServer>,

    // Hooks (placeholder for future tasks)
    /// Hook event handlers
    pub hooks: HashMap<HookEvent, Vec<HookMatcher>>,

    // Subagents (placeholder for future tasks)
    /// Agent definitions
    pub agents: HashMap<String, AgentDefinition>,

    // Session
    /// Session ID to resume
    pub resume: Option<String>,
    /// Continue the most recent conversation without specifying a session ID
    ///
    /// When true, passes `--continue` to the CLI. This is the most ergonomic
    /// way to resume work without tracking session IDs manually.
    pub continue_conversation: bool,
    /// Fork existing session
    pub fork_session: bool,
    /// Session name
    pub session_name: Option<String>,
    /// Enable file-based checkpointing
    pub enable_file_checkpointing: bool,

    // Environment
    /// Working directory
    pub cwd: Option<PathBuf>,
    /// Claude CLI executable path
    pub cli_path: Option<PathBuf>,
    /// Environment variables
    pub env: HashMap<String, String>,

    // Settings isolation
    /// Settings sources for configuration
    ///
    /// Matches the Python SDK field name `setting_sources`.
    /// The CLI flag is `--setting-sources`.
    pub setting_sources: Option<Vec<String>>,

    // Output
    /// Output format specification
    pub output_format: Option<serde_json::Value>,
    /// Include partial messages in output
    pub include_partial_messages: bool,

    // Advanced
    /// Beta features to enable (each passed as `--beta <name>`)
    pub betas: Vec<String>,
    /// Sandbox settings for restricting the agent's execution environment
    pub sandbox_settings: Option<SandboxSettings>,
    /// Maximum API spend in USD for this session (`--max-budget-usd <value>`)
    ///
    /// Critical for production deployments to prevent runaway costs from agentic loops.
    pub max_budget_usd: Option<f64>,
    /// Maximum reasoning/thinking token budget (`--max-thinking-tokens <value>`)
    ///
    /// Controls the balance between quality and cost when using extended thinking models.
    pub max_thinking_tokens: Option<u64>,

    /// Additional directories Claude can access (one `--add-dir` flag per entry)
    ///
    /// Useful for multi-repo setups or when working outside the CWD.
    pub add_dirs: Vec<PathBuf>,

    /// User identifier for audit logging and per-user permission policies (`--user <value>`)
    pub user: Option<String>,

    /// Fallback model if the primary model is unavailable (`--fallback-model <value>`)
    pub fallback_model: Option<String>,

    /// Stdout buffer size for the subprocess (None = system default)
    pub max_buffer_size: Option<usize>,

    /// Escape hatch for arbitrary CLI flags not yet modeled in the SDK.
    ///
    /// Each entry `(key, value)` is emitted as `--key value` or `--key` (if `value` is `None`).
    /// Enables forward compatibility when the CLI adds new flags before the SDK is updated.
    pub extra_args: Vec<(String, Option<String>)>,

    /// Callback invoked for each line of CLI stderr output.
    ///
    /// Useful for debugging and surfacing Claude CLI warnings in production.
    #[allow(clippy::type_complexity)]
    pub stderr_callback: Option<std::sync::Arc<dyn Fn(String) + Send + Sync>>,

    // Permission handler
    /// Optional permission handler for controlling tool execution.
    ///
    /// When set, this handler is registered before session initialization,
    /// ensuring it is in place before the first tool request arrives.
    /// This is more reliable than calling `register_can_use_tool_handler()`
    /// separately after `connect()`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use rusty_claw::prelude::*;
    /// use rusty_claw::permissions::PermissionDecision;
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
    ///     ) -> Result<PermissionDecision, ClawError> {
    ///         Ok(PermissionDecision::Allow { updated_input: None })
    ///     }
    /// }
    ///
    /// let options = ClaudeAgentOptions::builder()
    ///     .permission_handler(MyHandler)
    ///     .build();
    /// ```
    pub permission_handler: Option<Arc<dyn CanUseToolHandler>>,
}

impl std::fmt::Debug for ClaudeAgentOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClaudeAgentOptions")
            .field("system_prompt", &self.system_prompt)
            .field("append_system_prompt", &self.append_system_prompt)
            .field("max_turns", &self.max_turns)
            .field("model", &self.model)
            .field("allowed_tools", &self.allowed_tools)
            .field("disallowed_tools", &self.disallowed_tools)
            .field("permission_mode", &self.permission_mode)
            .field(
                "permission_prompt_tool_allowlist",
                &self.permission_prompt_tool_allowlist,
            )
            .field("mcp_servers", &"<McpServerConfig map>")
            .field("sdk_mcp_servers", &self.sdk_mcp_servers)
            .field("hooks", &self.hooks)
            .field("agents", &self.agents)
            .field("resume", &self.resume)
            .field("fork_session", &self.fork_session)
            .field("continue_conversation", &self.continue_conversation)
            .field("session_name", &self.session_name)
            .field("enable_file_checkpointing", &self.enable_file_checkpointing)
            .field("cwd", &self.cwd)
            .field("cli_path", &self.cli_path)
            .field("env", &self.env)
            .field("setting_sources", &self.setting_sources)
            .field("output_format", &self.output_format)
            .field("include_partial_messages", &self.include_partial_messages)
            .field("betas", &self.betas)
            .field("sandbox_settings", &self.sandbox_settings)
            .field("max_budget_usd", &self.max_budget_usd)
            .field("max_thinking_tokens", &self.max_thinking_tokens)
            .field("add_dirs", &self.add_dirs)
            .field("user", &self.user)
            .field("fallback_model", &self.fallback_model)
            .field("max_buffer_size", &self.max_buffer_size)
            .field("extra_args", &self.extra_args)
            .field(
                "stderr_callback",
                &self.stderr_callback.as_ref().map(|_| "<Fn(String)>"),
            )
            .field(
                "permission_handler",
                &self
                    .permission_handler
                    .as_ref()
                    .map(|_| "<CanUseToolHandler>"),
            )
            .finish()
    }
}

impl Clone for ClaudeAgentOptions {
    /// Clone options. `stderr_callback` is cloned via `Arc` — both copies share the same closure.
    fn clone(&self) -> Self {
        Self {
            system_prompt: self.system_prompt.clone(),
            append_system_prompt: self.append_system_prompt.clone(),
            max_turns: self.max_turns,
            model: self.model.clone(),
            allowed_tools: self.allowed_tools.clone(),
            disallowed_tools: self.disallowed_tools.clone(),
            permission_mode: self.permission_mode.clone(),
            permission_prompt_tool_allowlist: self.permission_prompt_tool_allowlist.clone(),
            mcp_servers: self.mcp_servers.clone(),
            sdk_mcp_servers: self.sdk_mcp_servers.clone(),
            hooks: self.hooks.clone(),
            agents: self.agents.clone(),
            resume: self.resume.clone(),
            fork_session: self.fork_session,
            continue_conversation: self.continue_conversation,
            session_name: self.session_name.clone(),
            enable_file_checkpointing: self.enable_file_checkpointing,
            cwd: self.cwd.clone(),
            cli_path: self.cli_path.clone(),
            env: self.env.clone(),
            setting_sources: self.setting_sources.clone(),
            output_format: self.output_format.clone(),
            include_partial_messages: self.include_partial_messages,
            betas: self.betas.clone(),
            sandbox_settings: self.sandbox_settings.clone(),
            max_budget_usd: self.max_budget_usd,
            max_thinking_tokens: self.max_thinking_tokens,
            add_dirs: self.add_dirs.clone(),
            user: self.user.clone(),
            fallback_model: self.fallback_model.clone(),
            max_buffer_size: self.max_buffer_size,
            extra_args: self.extra_args.clone(),
            stderr_callback: self.stderr_callback.clone(),
            permission_handler: self.permission_handler.clone(),
        }
    }
}

impl ClaudeAgentOptions {
    /// Create a new options builder
    ///
    /// # Example
    ///
    /// ```
    /// use rusty_claw::options::ClaudeAgentOptions;
    ///
    /// let options = ClaudeAgentOptions::builder()
    ///     .max_turns(10)
    ///     .build();
    /// ```
    pub fn builder() -> ClaudeAgentOptionsBuilder {
        ClaudeAgentOptionsBuilder::default()
    }

    /// Serialize `mcp_servers` to the JSON format expected by `--mcp-config`.
    ///
    /// Returns `None` when `mcp_servers` is empty (no `--mcp-config` flag needed).
    ///
    /// The output format is:
    /// ```json
    /// {
    ///   "mcpServers": {
    ///     "my_server": { "type": "stdio", "command": "npx", "args": [...] }
    ///   }
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a [`serde_json::Error`] if serialization fails.
    ///
    /// # Example
    ///
    /// ```
    /// use rusty_claw::options::{ClaudeAgentOptions, McpServerConfig, McpStdioServerConfig};
    /// use std::collections::HashMap;
    ///
    /// let mut servers = HashMap::new();
    /// servers.insert("fs".to_string(), McpServerConfig::Stdio(McpStdioServerConfig {
    ///     command: "npx".to_string(),
    ///     args: vec!["-y".to_string(), "@modelcontextprotocol/server-filesystem".to_string()],
    ///     env: HashMap::new(),
    /// }));
    ///
    /// let options = ClaudeAgentOptions::builder()
    ///     .mcp_servers(servers)
    ///     .build();
    ///
    /// let json = options.to_mcp_config_json().unwrap().unwrap();
    /// assert!(json.contains("\"mcpServers\""));
    /// assert!(json.contains("\"type\":\"stdio\""));
    /// ```
    pub fn to_mcp_config_json(&self) -> Result<Option<String>, serde_json::Error> {
        if self.mcp_servers.is_empty() {
            return Ok(None);
        }
        let payload = serde_json::json!({ "mcpServers": &self.mcp_servers });
        Ok(Some(serde_json::to_string(&payload)?))
    }

    /// Convert options to Claude CLI base arguments (without the prompt)
    ///
    /// This method generates CLI arguments compatible with the Claude CLI,
    /// following SPEC.md section 2.2. It produces all arguments **except**
    /// the `-p <prompt>` flag so the same logic can be shared between the
    /// one-shot `query()` path and the interactive `ClaudeClient::connect()`
    /// path (which uses `--input-format stream-json` instead).
    ///
    /// # Example
    ///
    /// ```
    /// use rusty_claw::options::{ClaudeAgentOptions, PermissionMode};
    ///
    /// let options = ClaudeAgentOptions::builder()
    ///     .max_turns(5)
    ///     .permission_mode(PermissionMode::AcceptEdits)
    ///     .build();
    ///
    /// let args = options.to_base_cli_args();
    /// assert!(args.contains(&"--max-turns".to_string()));
    /// assert!(args.contains(&"5".to_string()));
    /// // No "-p" flag — caller appends the prompt or --input-format as needed
    /// assert!(!args.contains(&"-p".to_string()));
    /// ```
    pub fn to_base_cli_args(&self) -> Vec<String> {
        let mut args = vec![
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--verbose".to_string(),
        ];

        // Max turns
        if let Some(max_turns) = self.max_turns {
            args.push("--max-turns".to_string());
            args.push(max_turns.to_string());
        }

        // Model
        if let Some(model) = &self.model {
            args.push("--model".to_string());
            args.push(model.clone());
        }

        // Permission mode
        if let Some(mode) = &self.permission_mode {
            args.push("--permission-mode".to_string());
            args.push(mode.to_cli_arg().to_string());
        }

        // System prompt
        if let Some(sys_prompt) = &self.system_prompt {
            match sys_prompt {
                SystemPrompt::Custom(text) => {
                    args.push("--system-prompt".to_string());
                    args.push(text.clone());
                }
                SystemPrompt::Preset { preset } => {
                    args.push("--system-prompt-preset".to_string());
                    args.push(preset.clone());
                }
            }
        }

        // Append system prompt
        if let Some(append) = &self.append_system_prompt {
            args.push("--append-system-prompt".to_string());
            args.push(append.clone());
        }

        // Allowed tools
        if !self.allowed_tools.is_empty() {
            args.push("--allowed-tools".to_string());
            args.push(self.allowed_tools.join(","));
        }

        // Disallowed tools
        if !self.disallowed_tools.is_empty() {
            args.push("--disallowed-tools".to_string());
            args.push(self.disallowed_tools.join(","));
        }

        // Permission prompt tool allowlist
        for tool in &self.permission_prompt_tool_allowlist {
            args.push("--permission-prompt-tool-name".to_string());
            args.push(tool.clone());
        }

        // Beta features
        for beta in &self.betas {
            args.push("--beta".to_string());
            args.push(beta.clone());
        }

        // Session options
        if let Some(resume) = &self.resume {
            args.push("--resume".to_string());
            args.push(resume.clone());
        }

        if self.continue_conversation {
            args.push("--continue".to_string());
        }

        if self.fork_session {
            args.push("--fork-session".to_string());
        }

        if let Some(name) = &self.session_name {
            args.push("--session-name".to_string());
            args.push(name.clone());
        }

        if self.enable_file_checkpointing {
            args.push("--enable-file-checkpointing".to_string());
        }

        // Settings isolation for reproducibility
        match &self.setting_sources {
            Some(sources) => {
                args.push("--setting-sources".to_string());
                args.push(sources.join(","));
            }
            None => {
                args.push("--setting-sources".to_string());
                args.push(String::new());
            }
        }

        // Budget cap
        if let Some(max_budget) = self.max_budget_usd {
            args.push("--max-budget-usd".to_string());
            args.push(max_budget.to_string());
        }

        // Thinking token limit
        if let Some(max_thinking) = self.max_thinking_tokens {
            args.push("--max-thinking-tokens".to_string());
            args.push(max_thinking.to_string());
        }

        // Sandbox settings
        if let Some(sandbox) = &self.sandbox_settings {
            if sandbox.enabled {
                args.push("--sandbox".to_string());
            } else {
                args.push("--no-sandbox".to_string());
            }

            if sandbox.auto_allow_bash_if_sandboxed {
                args.push("--auto-allow-bash-if-sandboxed".to_string());
            }

            for cmd in &sandbox.excluded_commands {
                args.push("--excluded-command".to_string());
                args.push(cmd.clone());
            }

            for cmd in &sandbox.allow_unsandboxed_commands {
                args.push("--allow-unsandboxed-command".to_string());
                args.push(cmd.clone());
            }

            if let Some(network) = sandbox.network {
                if network {
                    args.push("--sandbox-network".to_string());
                } else {
                    args.push("--no-sandbox-network".to_string());
                }
            }
        }

        // Note: --input-format=stream-json is NOT included here because this method
        // is used by the one-shot query() API which closes stdin immediately.
        // The interactive ClaudeClient adds --input-format=stream-json separately
        // in its connect() method, where it sends a proper initialization message.

        // Additional directories
        for dir in &self.add_dirs {
            args.push("--add-dir".to_string());
            args.push(dir.to_string_lossy().into_owned());
        }

        // User identifier
        if let Some(user) = &self.user {
            args.push("--user".to_string());
            args.push(user.clone());
        }

        // Fallback model
        if let Some(fallback) = &self.fallback_model {
            args.push("--fallback-model".to_string());
            args.push(fallback.clone());
        }

        // Extra args escape hatch
        for (key, value) in &self.extra_args {
            let flag = if key.starts_with("--") {
                key.clone()
            } else {
                format!("--{}", key)
            };
            args.push(flag);
            if let Some(val) = value {
                args.push(val.clone());
            }
        }

        // External MCP servers (stdio/SSE/HTTP) via --mcp-config inline JSON.
        // NOTE: SDK-hosted servers (SdkMcpServerImpl) are excluded here — the CLI hangs
        // when type:"sdk" entries appear in --mcp-config. SDK servers are wired via the
        // sdkMcpServers field in the initialize control request instead.
        if let Ok(Some(mcp_json)) = self.to_mcp_config_json() {
            args.push("--mcp-config".to_string());
            args.push(mcp_json);
        }

        args
    }

    /// Convert options to Claude CLI arguments for one-shot query mode
    ///
    /// Calls [`Self::to_base_cli_args()`] and appends `-p <prompt>` at the end.
    ///
    /// # Arguments
    ///
    /// * `prompt` - The user prompt to include in arguments
    ///
    /// # Example
    ///
    /// ```
    /// use rusty_claw::options::{ClaudeAgentOptions, PermissionMode};
    ///
    /// let options = ClaudeAgentOptions::builder()
    ///     .max_turns(5)
    ///     .permission_mode(PermissionMode::AcceptEdits)
    ///     .build();
    ///
    /// let args = options.to_cli_args("test prompt");
    /// assert!(args.contains(&"--max-turns".to_string()));
    /// assert!(args.contains(&"5".to_string()));
    /// assert!(args.contains(&"-p".to_string()));
    /// ```
    pub fn to_cli_args(&self, prompt: &str) -> Vec<String> {
        let mut args = self.to_base_cli_args();
        // Prompt
        args.push("-p".to_string());
        args.push(prompt.to_string());
        args
    }
}

/// Builder for [`ClaudeAgentOptions`]
///
/// Provides a fluent interface for constructing options with chainable setters.
///
/// # Example
///
/// ```
/// use rusty_claw::options::{ClaudeAgentOptions, PermissionMode};
///
/// let options = ClaudeAgentOptions::builder()
///     .max_turns(5)
///     .model("claude-sonnet-4")
///     .permission_mode(PermissionMode::AcceptEdits)
///     .allowed_tools(vec!["Read".to_string(), "Bash".to_string()])
///     .build();
/// ```
#[derive(Debug, Default)]
pub struct ClaudeAgentOptionsBuilder {
    inner: ClaudeAgentOptions,
}

impl ClaudeAgentOptionsBuilder {
    /// Set system prompt
    pub fn system_prompt(mut self, prompt: SystemPrompt) -> Self {
        self.inner.system_prompt = Some(prompt);
        self
    }

    /// Set text to append to system prompt
    pub fn append_system_prompt(mut self, text: impl Into<String>) -> Self {
        self.inner.append_system_prompt = Some(text.into());
        self
    }

    /// Set maximum conversation turns
    pub fn max_turns(mut self, turns: u32) -> Self {
        self.inner.max_turns = Some(turns);
        self
    }

    /// Set model identifier
    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.inner.model = Some(model.into());
        self
    }

    /// Set allowed tools
    pub fn allowed_tools(mut self, tools: Vec<String>) -> Self {
        self.inner.allowed_tools = tools;
        self
    }

    /// Set disallowed tools
    pub fn disallowed_tools(mut self, tools: Vec<String>) -> Self {
        self.inner.disallowed_tools = tools;
        self
    }

    /// Set permission mode
    pub fn permission_mode(mut self, mode: PermissionMode) -> Self {
        self.inner.permission_mode = Some(mode);
        self
    }

    /// Set permission prompt tool allowlist
    pub fn permission_prompt_tool_allowlist(mut self, tools: Vec<String>) -> Self {
        self.inner.permission_prompt_tool_allowlist = tools;
        self
    }

    /// Set MCP servers
    pub fn mcp_servers(mut self, servers: HashMap<String, McpServerConfig>) -> Self {
        self.inner.mcp_servers = servers;
        self
    }

    /// Set SDK MCP servers
    pub fn sdk_mcp_servers(mut self, servers: Vec<SdkMcpServer>) -> Self {
        self.inner.sdk_mcp_servers = servers;
        self
    }

    /// Set hooks
    pub fn hooks(mut self, hooks: HashMap<HookEvent, Vec<HookMatcher>>) -> Self {
        self.inner.hooks = hooks;
        self
    }

    /// Set agents
    pub fn agents(mut self, agents: HashMap<String, AgentDefinition>) -> Self {
        self.inner.agents = agents;
        self
    }

    /// Set session ID to resume
    pub fn resume(mut self, session_id: impl Into<String>) -> Self {
        self.inner.resume = Some(session_id.into());
        self
    }

    /// Continue the most recent conversation without specifying a session ID
    ///
    /// When `true`, passes `--continue` to the CLI. More ergonomic than tracking session IDs.
    pub fn continue_conversation(mut self, continue_conv: bool) -> Self {
        self.inner.continue_conversation = continue_conv;
        self
    }

    /// Enable session forking
    pub fn fork_session(mut self, fork: bool) -> Self {
        self.inner.fork_session = fork;
        self
    }

    /// Set session name
    pub fn session_name(mut self, name: impl Into<String>) -> Self {
        self.inner.session_name = Some(name.into());
        self
    }

    /// Enable file-based checkpointing
    pub fn enable_file_checkpointing(mut self, enable: bool) -> Self {
        self.inner.enable_file_checkpointing = enable;
        self
    }

    /// Set working directory
    pub fn cwd(mut self, path: impl Into<PathBuf>) -> Self {
        self.inner.cwd = Some(path.into());
        self
    }

    /// Set Claude CLI executable path
    pub fn cli_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.inner.cli_path = Some(path.into());
        self
    }

    /// Set environment variables
    pub fn env(mut self, env: HashMap<String, String>) -> Self {
        self.inner.env = env;
        self
    }

    /// Set settings sources
    /// Set settings sources
    ///
    /// Matches the Python SDK's `setting_sources` field name.
    /// The CLI flag is `--setting-sources`.
    pub fn setting_sources(mut self, sources: Vec<String>) -> Self {
        self.inner.setting_sources = Some(sources);
        self
    }

    /// Set settings sources (deprecated name, use `setting_sources` instead)
    ///
    /// This builder method is kept for backward compatibility.
    /// Prefer [`ClaudeAgentOptionsBuilder::setting_sources`] in new code.
    #[deprecated(
        since = "0.4.0",
        note = "Renamed to `setting_sources` to match Python SDK"
    )]
    #[allow(deprecated)]
    pub fn settings_sources(mut self, sources: Vec<String>) -> Self {
        self.inner.setting_sources = Some(sources);
        self
    }

    /// Set output format
    pub fn output_format(mut self, format: serde_json::Value) -> Self {
        self.inner.output_format = Some(format);
        self
    }

    /// Enable partial messages in output
    pub fn include_partial_messages(mut self, include: bool) -> Self {
        self.inner.include_partial_messages = include;
        self
    }

    /// Set beta features to enable
    ///
    /// Each feature name is passed as a separate `--beta <name>` argument.
    pub fn betas(mut self, betas: Vec<String>) -> Self {
        self.inner.betas = betas;
        self
    }

    /// Set sandbox settings
    pub fn sandbox_settings(mut self, settings: SandboxSettings) -> Self {
        self.inner.sandbox_settings = Some(settings);
        self
    }

    /// Set maximum API spend in USD
    ///
    /// Prevents runaway costs from agentic loops in production deployments.
    pub fn max_budget_usd(mut self, budget: f64) -> Self {
        self.inner.max_budget_usd = Some(budget);
        self
    }

    /// Set maximum thinking token budget
    ///
    /// Controls the balance between quality and cost when using extended thinking models.
    pub fn max_thinking_tokens(mut self, tokens: u64) -> Self {
        self.inner.max_thinking_tokens = Some(tokens);
        self
    }

    /// Set additional directories Claude can access
    ///
    /// Each directory is passed as a separate `--add-dir` flag to the CLI.
    pub fn add_dirs(mut self, dirs: impl IntoIterator<Item = impl Into<PathBuf>>) -> Self {
        self.inner.add_dirs = dirs.into_iter().map(Into::into).collect();
        self
    }

    /// Add a single additional directory Claude can access
    pub fn add_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.inner.add_dirs.push(dir.into());
        self
    }

    /// Set user identifier for audit logging
    pub fn user(mut self, user: impl Into<String>) -> Self {
        self.inner.user = Some(user.into());
        self
    }

    /// Set fallback model if primary model is unavailable
    pub fn fallback_model(mut self, model: impl Into<String>) -> Self {
        self.inner.fallback_model = Some(model.into());
        self
    }

    /// Set buffer size for subprocess stdout
    pub fn max_buffer_size(mut self, size: usize) -> Self {
        self.inner.max_buffer_size = Some(size);
        self
    }

    /// Set extra CLI args escape hatch
    ///
    /// Each entry `(key, value)` is emitted as `--key value` or `--key` (if `value` is `None`).
    pub fn extra_args(mut self, args: impl IntoIterator<Item = (String, Option<String>)>) -> Self {
        self.inner.extra_args = args.into_iter().collect();
        self
    }

    /// Add a single extra CLI arg
    pub fn extra_arg(mut self, key: impl Into<String>, value: Option<String>) -> Self {
        self.inner.extra_args.push((key.into(), value));
        self
    }

    /// Set stderr callback for observing CLI stderr output
    ///
    /// The callback is invoked for each line of CLI stderr output (without trailing newline).
    pub fn stderr_callback(mut self, callback: impl Fn(String) + Send + Sync + 'static) -> Self {
        self.inner.stderr_callback = Some(std::sync::Arc::new(callback));
        self
    }

    /// Set the permission handler for controlling tool execution.
    ///
    /// The handler is registered before session initialization, ensuring it
    /// is in place before the first can_use_tool request arrives from the CLI.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use rusty_claw::prelude::*;
    /// use rusty_claw::permissions::{DefaultPermissionHandler, PermissionDecision};
    /// use rusty_claw::options::PermissionMode;
    ///
    /// let handler = DefaultPermissionHandler::builder()
    ///     .mode(PermissionMode::Deny)
    ///     .allowed_tools(vec!["Read".to_string()])
    ///     .build();
    ///
    /// let options = ClaudeAgentOptions::builder()
    ///     .permission_handler(handler)
    ///     .build();
    /// ```
    pub fn permission_handler(mut self, handler: impl CanUseToolHandler + 'static) -> Self {
        self.inner.permission_handler = Some(Arc::new(handler));
        self
    }

    /// Build the options
    pub fn build(self) -> ClaudeAgentOptions {
        self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_default() {
        let opts = ClaudeAgentOptions::builder().build();
        assert_eq!(opts.max_turns, None);
        assert_eq!(opts.model, None);
        assert!(opts.allowed_tools.is_empty());
        assert!(opts.disallowed_tools.is_empty());
        assert!(opts.mcp_servers.is_empty());
        assert!(opts.hooks.is_empty());
        assert!(opts.agents.is_empty());
        assert!(!opts.fork_session);
        assert!(!opts.continue_conversation);
        assert!(!opts.enable_file_checkpointing);
        assert!(!opts.include_partial_messages);
        assert_eq!(opts.max_budget_usd, None);
        assert_eq!(opts.max_thinking_tokens, None);
        assert!(opts.sandbox_settings.is_none());
    }

    #[test]
    fn test_builder_chaining() {
        let opts = ClaudeAgentOptions::builder()
            .max_turns(5)
            .model("claude-sonnet-4")
            .allowed_tools(vec!["Read".to_string(), "Bash".to_string()])
            .permission_mode(PermissionMode::AcceptEdits)
            .build();

        assert_eq!(opts.max_turns, Some(5));
        assert_eq!(opts.model, Some("claude-sonnet-4".to_string()));
        assert_eq!(opts.allowed_tools.len(), 2);
        assert!(matches!(
            opts.permission_mode,
            Some(PermissionMode::AcceptEdits)
        ));
    }

    #[test]
    fn test_builder_all_fields() {
        let opts = ClaudeAgentOptions::builder()
            .system_prompt(SystemPrompt::Custom("test".to_string()))
            .append_system_prompt("append")
            .max_turns(10)
            .model("claude-opus-4")
            .allowed_tools(vec!["Read".to_string()])
            .disallowed_tools(vec!["Bash".to_string()])
            .permission_mode(PermissionMode::Plan)
            .permission_prompt_tool_allowlist(vec!["Edit".to_string()])
            .resume("session-123")
            .continue_conversation(true)
            .fork_session(true)
            .session_name("test-session")
            .enable_file_checkpointing(true)
            .cwd("/tmp")
            .include_partial_messages(true)
            .betas(vec!["feature-1".to_string()])
            .max_budget_usd(5.0)
            .max_thinking_tokens(10000)
            .build();

        assert!(opts.system_prompt.is_some());
        assert_eq!(opts.append_system_prompt, Some("append".to_string()));
        assert_eq!(opts.max_turns, Some(10));
        assert_eq!(opts.model, Some("claude-opus-4".to_string()));
        assert_eq!(opts.allowed_tools, vec!["Read".to_string()]);
        assert_eq!(opts.disallowed_tools, vec!["Bash".to_string()]);
        assert!(matches!(opts.permission_mode, Some(PermissionMode::Plan)));
        assert_eq!(
            opts.permission_prompt_tool_allowlist,
            vec!["Edit".to_string()]
        );
        assert_eq!(opts.resume, Some("session-123".to_string()));
        assert!(opts.continue_conversation);
        assert!(opts.fork_session);
        assert_eq!(opts.session_name, Some("test-session".to_string()));
        assert!(opts.enable_file_checkpointing);
        assert!(opts.cwd.is_some());
        assert!(opts.include_partial_messages);
        assert_eq!(opts.betas, vec!["feature-1".to_string()]);
        assert_eq!(opts.max_budget_usd, Some(5.0));
        assert_eq!(opts.max_thinking_tokens, Some(10000));
    }

    #[test]
    fn test_to_cli_args_minimal() {
        let opts = ClaudeAgentOptions::default();
        let args = opts.to_cli_args("test prompt");

        assert!(args.contains(&"--output-format".to_string()));
        assert!(args.contains(&"stream-json".to_string()));
        assert!(args.contains(&"--verbose".to_string()));
        assert!(args.contains(&"--setting-sources".to_string()));
        assert!(args.contains(&"-p".to_string()));
        assert!(args.contains(&"test prompt".to_string()));
    }

    #[test]
    fn test_to_cli_args_with_options() {
        let opts = ClaudeAgentOptions::builder()
            .max_turns(10)
            .model("claude-opus-4")
            .permission_mode(PermissionMode::Plan)
            .build();

        let args = opts.to_cli_args("test");

        // All args use space-separated format (--flag value), not --flag=value
        assert!(args.contains(&"--max-turns".to_string()));
        assert!(args.contains(&"10".to_string()));
        assert!(args.contains(&"--model".to_string()));
        assert!(args.contains(&"claude-opus-4".to_string()));
        assert!(args.contains(&"--permission-mode".to_string()));
        assert!(args.contains(&"plan".to_string()));
    }

    #[test]
    fn test_to_cli_args_system_prompt_custom() {
        let opts = ClaudeAgentOptions::builder()
            .system_prompt(SystemPrompt::Custom("You are a helper".to_string()))
            .build();

        let args = opts.to_cli_args("test");

        assert!(args.contains(&"--system-prompt".to_string()));
        assert!(args.contains(&"You are a helper".to_string()));
    }

    #[test]
    fn test_to_cli_args_system_prompt_preset() {
        let opts = ClaudeAgentOptions::builder()
            .system_prompt(SystemPrompt::Preset {
                preset: "assistant".to_string(),
            })
            .build();

        let args = opts.to_cli_args("test");

        assert!(args.contains(&"--system-prompt-preset".to_string()));
        assert!(args.contains(&"assistant".to_string()));
    }

    #[test]
    fn test_to_cli_args_allowed_tools() {
        let opts = ClaudeAgentOptions::builder()
            .allowed_tools(vec!["Read".to_string(), "Bash".to_string()])
            .build();

        let args = opts.to_cli_args("test");

        assert!(args.contains(&"--allowed-tools".to_string()));
        assert!(args.contains(&"Read,Bash".to_string()));
    }

    #[test]
    fn test_to_cli_args_disallowed_tools() {
        let opts = ClaudeAgentOptions::builder()
            .disallowed_tools(vec!["Edit".to_string(), "Write".to_string()])
            .build();

        let args = opts.to_cli_args("test");

        assert!(args.contains(&"--disallowed-tools".to_string()));
        assert!(args.contains(&"Edit,Write".to_string()));
    }

    #[test]
    fn test_to_cli_args_session_options() {
        let opts = ClaudeAgentOptions::builder()
            .resume("session-123")
            .fork_session(true)
            .session_name("my-session")
            .enable_file_checkpointing(true)
            .build();

        let args = opts.to_cli_args("test");

        assert!(args.contains(&"--resume".to_string()));
        assert!(args.contains(&"session-123".to_string()));
        assert!(args.contains(&"--fork-session".to_string()));
        assert!(args.contains(&"--session-name".to_string()));
        assert!(args.contains(&"my-session".to_string()));
        assert!(args.contains(&"--enable-file-checkpointing".to_string()));
    }

    #[test]
    fn test_to_cli_args_setting_sources_default() {
        // When no setting_sources configured, should emit empty --setting-sources
        let opts = ClaudeAgentOptions::default();
        let args = opts.to_cli_args("test");
        assert!(args.contains(&"--setting-sources".to_string()));
        // The value after --setting-sources should be an empty string
        let idx = args.iter().position(|a| a == "--setting-sources").unwrap();
        assert_eq!(args[idx + 1], "");
    }

    #[test]
    fn test_to_cli_args_setting_sources_custom() {
        // When setting_sources configured, should use the user's values
        let opts = ClaudeAgentOptions::builder()
            .setting_sources(vec!["local".to_string(), "project".to_string()])
            .build();
        let args = opts.to_cli_args("test");
        assert!(args.contains(&"--setting-sources".to_string()));
        assert!(args.contains(&"local,project".to_string()));
    }

    #[test]
    fn test_permission_mode_to_cli_arg() {
        assert_eq!(PermissionMode::Default.to_cli_arg(), "default");
        assert_eq!(PermissionMode::AcceptEdits.to_cli_arg(), "acceptEdits");
        assert_eq!(
            PermissionMode::BypassPermissions.to_cli_arg(),
            "bypassPermissions"
        );
        assert_eq!(PermissionMode::Plan.to_cli_arg(), "plan");
    }

    #[test]
    fn test_default_trait() {
        let opts = ClaudeAgentOptions::default();
        assert!(opts.system_prompt.is_none());
        assert!(opts.max_turns.is_none());
        assert!(opts.model.is_none());
        assert!(opts.allowed_tools.is_empty());
        assert!(opts.permission_mode.is_none());
        assert!(opts.resume.is_none());
        assert!(!opts.fork_session);
        assert!(!opts.continue_conversation);
        assert!(opts.max_budget_usd.is_none());
        assert!(opts.max_thinking_tokens.is_none());
        assert!(opts.sandbox_settings.is_none());
    }

    #[test]
    fn test_collections_handling() {
        let mut env = HashMap::new();
        env.insert("KEY".to_string(), "value".to_string());

        let opts = ClaudeAgentOptions::builder().env(env.clone()).build();

        assert_eq!(opts.env, env);
    }

    #[test]
    fn test_pathbuf_conversion() {
        let opts = ClaudeAgentOptions::builder()
            .cwd("/tmp/test")
            .cli_path("/usr/bin/claude")
            .build();

        assert_eq!(opts.cwd, Some(PathBuf::from("/tmp/test")));
        assert_eq!(opts.cli_path, Some(PathBuf::from("/usr/bin/claude")));
    }

    #[test]
    fn test_to_cli_args_permission_prompt_tool_allowlist() {
        let opts = ClaudeAgentOptions::builder()
            .permission_prompt_tool_allowlist(vec!["Bash".to_string(), "Edit".to_string()])
            .build();

        let args = opts.to_cli_args("test");

        // Each tool gets its own --permission-prompt-tool-name flag
        let tool_flags: Vec<_> = args
            .iter()
            .enumerate()
            .filter(|(_, a)| a.as_str() == "--permission-prompt-tool-name")
            .map(|(i, _)| args[i + 1].as_str())
            .collect();
        assert_eq!(tool_flags, vec!["Bash", "Edit"]);
    }

    #[test]
    fn test_to_cli_args_betas() {
        let opts = ClaudeAgentOptions::builder()
            .betas(vec![
                "beta-feature-1".to_string(),
                "beta-feature-2".to_string(),
            ])
            .build();

        let args = opts.to_cli_args("test");

        // Each beta gets its own --beta flag
        let beta_flags: Vec<_> = args
            .iter()
            .enumerate()
            .filter(|(_, a)| a.as_str() == "--beta")
            .map(|(i, _)| args[i + 1].as_str())
            .collect();
        assert_eq!(beta_flags, vec!["beta-feature-1", "beta-feature-2"]);
    }

    #[test]
    fn test_to_cli_args_continue_conversation() {
        let opts = ClaudeAgentOptions::builder()
            .continue_conversation(true)
            .build();
        let args = opts.to_cli_args("test");
        assert!(args.contains(&"--continue".to_string()));
    }

    #[test]
    fn test_to_cli_args_continue_false() {
        let opts = ClaudeAgentOptions::builder()
            .continue_conversation(false)
            .build();
        let args = opts.to_cli_args("test");
        assert!(!args.contains(&"--continue".to_string()));
    }

    #[test]
    fn test_to_cli_args_max_budget_usd() {
        let opts = ClaudeAgentOptions::builder().max_budget_usd(10.5).build();
        let args = opts.to_cli_args("test");
        assert!(args.contains(&"--max-budget-usd".to_string()));
        assert!(args.contains(&"10.5".to_string()));
    }

    #[test]
    fn test_to_cli_args_max_thinking_tokens() {
        let opts = ClaudeAgentOptions::builder()
            .max_thinking_tokens(8000)
            .build();
        let args = opts.to_cli_args("test");
        assert!(args.contains(&"--max-thinking-tokens".to_string()));
        assert!(args.contains(&"8000".to_string()));
    }

    #[test]
    fn test_to_cli_args_sandbox_enabled() {
        let opts = ClaudeAgentOptions::builder()
            .sandbox_settings(SandboxSettings::enabled())
            .build();
        let args = opts.to_cli_args("test");
        assert!(args.contains(&"--sandbox".to_string()));
        assert!(!args.contains(&"--no-sandbox".to_string()));
    }

    #[test]
    fn test_to_cli_args_sandbox_disabled() {
        let opts = ClaudeAgentOptions::builder()
            .sandbox_settings(SandboxSettings::disabled())
            .build();
        let args = opts.to_cli_args("test");
        assert!(args.contains(&"--no-sandbox".to_string()));
        assert!(!args.contains(&"--sandbox".to_string()));
    }

    #[test]
    fn test_to_cli_args_sandbox_full_settings() {
        let settings = SandboxSettings {
            enabled: true,
            auto_allow_bash_if_sandboxed: true,
            excluded_commands: vec!["rm".to_string(), "sudo".to_string()],
            allow_unsandboxed_commands: vec!["git".to_string()],
            network: Some(false),
        };
        let opts = ClaudeAgentOptions::builder()
            .sandbox_settings(settings)
            .build();
        let args = opts.to_cli_args("test");
        assert!(args.contains(&"--sandbox".to_string()));
        assert!(args.contains(&"--auto-allow-bash-if-sandboxed".to_string()));
        assert!(args.contains(&"--excluded-command".to_string()));
        assert!(args.contains(&"rm".to_string()));
        assert!(args.contains(&"sudo".to_string()));
        assert!(args.contains(&"--allow-unsandboxed-command".to_string()));
        assert!(args.contains(&"git".to_string()));
        assert!(args.contains(&"--no-sandbox-network".to_string()));
    }

    #[test]
    fn test_to_cli_args_sandbox_network_allow() {
        let settings = SandboxSettings {
            enabled: true,
            auto_allow_bash_if_sandboxed: false,
            excluded_commands: vec![],
            allow_unsandboxed_commands: vec![],
            network: Some(true),
        };
        let opts = ClaudeAgentOptions::builder()
            .sandbox_settings(settings)
            .build();
        let args = opts.to_cli_args("test");
        assert!(args.contains(&"--sandbox-network".to_string()));
        assert!(!args.contains(&"--no-sandbox-network".to_string()));
    }

    #[test]
    fn test_sandbox_settings_constructors() {
        let enabled = SandboxSettings::enabled();
        assert!(enabled.enabled);
        assert!(!enabled.auto_allow_bash_if_sandboxed);
        assert!(enabled.excluded_commands.is_empty());
        assert!(enabled.allow_unsandboxed_commands.is_empty());
        assert!(enabled.network.is_none());

        let disabled = SandboxSettings::disabled();
        assert!(!disabled.enabled);
    }
    // =========================================================================
    // MCP server config tests (bvo, 9be, xik, yhn)
    // =========================================================================

    #[test]
    fn test_to_mcp_config_json_empty() {
        let opts = ClaudeAgentOptions::default();
        let result = opts.to_mcp_config_json().unwrap();
        assert!(result.is_none(), "Empty mcp_servers should produce None");
    }

    #[test]
    fn test_to_cli_args_no_mcp_config_when_empty() {
        let opts = ClaudeAgentOptions::default();
        let args = opts.to_cli_args("test");
        assert!(
            !args.contains(&"--mcp-config".to_string()),
            "--mcp-config should not be present when mcp_servers is empty"
        );
    }

    // HookMatcher tests
    #[test]
    fn test_hook_matcher_all_matches_any_tool() {
        let matcher = HookMatcher::all();
        assert!(matcher.matches("Bash"));
        assert!(matcher.matches("Read"));
        assert!(matcher.matches("mcp__text_tools__word_count"));
        assert!(matcher.matches(""));
    }

    #[test]
    fn test_hook_matcher_exact_name() {
        let matcher = HookMatcher::tool("Bash");
        assert!(matcher.matches("Bash"));
        assert!(!matcher.matches("Read"));
        assert!(!matcher.matches("BashTool"));
        assert!(!matcher.matches("bash")); // case-sensitive
    }

    #[test]
    fn test_hook_matcher_wildcard_prefix() {
        let matcher = HookMatcher::tool("mcp__*");
        assert!(matcher.matches("mcp__text_tools__word_count"));
        assert!(matcher.matches("mcp__server__tool"));
        assert!(matcher.matches("mcp__"));
        assert!(!matcher.matches("Bash"));
        assert!(!matcher.matches("mcp_underscore")); // single underscore, no match
    }

    #[test]
    fn test_hook_matcher_wildcard_suffix() {
        let matcher = HookMatcher::tool("*_tool");
        assert!(matcher.matches("my_tool"));
        assert!(matcher.matches("bash_tool"));
        assert!(!matcher.matches("tool_runner"));
    }

    #[test]
    fn test_hook_matcher_wildcard_only() {
        let matcher = HookMatcher::tool("*");
        assert!(matcher.matches("Bash"));
        assert!(matcher.matches("Read"));
        assert!(matcher.matches("anything"));
    }

    #[test]
    fn test_hook_matcher_wildcard_middle() {
        let matcher = HookMatcher::tool("mcp__*__tool");
        assert!(matcher.matches("mcp__server__tool"));
        assert!(!matcher.matches("mcp__tool")); // no middle segment
    }

    #[test]
    fn test_hook_matcher_timeout_ms() {
        let matcher = HookMatcher::all().with_timeout_ms(5000);
        assert_eq!(matcher.timeout_ms, Some(5000));

        let matcher = HookMatcher::all();
        assert_eq!(matcher.timeout_ms, None);
    }

    #[test]
    fn test_hook_matcher_with_events() {
        let matcher =
            HookMatcher::all().with_events(vec![HookEvent::PreToolUse, HookEvent::PostToolUse]);

        assert!(matcher.matches_event(&HookEvent::PreToolUse));
        assert!(matcher.matches_event(&HookEvent::PostToolUse));
        assert!(!matcher.matches_event(&HookEvent::Stop));
        assert!(!matcher.matches_event(&HookEvent::UserPromptSubmit));
    }

    #[test]
    fn test_hook_matcher_empty_events_matches_all() {
        let matcher = HookMatcher::all(); // no events set
        assert!(matcher.matches_event(&HookEvent::PreToolUse));
        assert!(matcher.matches_event(&HookEvent::PostToolUse));
        assert!(matcher.matches_event(&HookEvent::Stop));
        assert!(matcher.matches_event(&HookEvent::Notification));
    }

    #[test]
    fn test_hook_matcher_serialization_skips_empty_hooks() {
        let matcher = HookMatcher::tool("Bash");
        let json = serde_json::to_value(&matcher).unwrap();
        // hooks array should not appear when empty
        assert!(!json.as_object().unwrap().contains_key("hooks"));
        assert!(!json.as_object().unwrap().contains_key("timeout_ms"));
        assert_eq!(json["tool_name"], "Bash");
    }

    #[test]
    fn test_hook_matcher_serialization_with_all_fields() {
        let matcher = HookMatcher::tool("Bash")
            .with_events(vec![HookEvent::PreToolUse])
            .with_timeout_ms(3000);
        let json = serde_json::to_value(&matcher).unwrap();
        assert_eq!(json["tool_name"], "Bash");
        assert_eq!(json["timeout_ms"], 3000);
        assert!(json["hooks"].is_array());
    }
}

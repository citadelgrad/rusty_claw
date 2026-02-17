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

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// System prompt configuration
#[derive(Debug, Clone)]
pub enum SystemPrompt {
    /// Custom system prompt text
    Custom(String),
    /// Named preset system prompt
    Preset {
        /// Preset name
        preset: String
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

// Placeholder types for future implementation

/// MCP server configuration (placeholder for future MCP tasks)
#[derive(Debug, Clone)]
pub struct McpServerConfig {
    // Detailed implementation in future tasks (SPEC.md section 7.1)
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

/// Hook matcher for pattern-based hook triggering
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
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookMatcher {
    /// Tool name pattern to match (e.g., "Bash", "mcp__*", or None for all)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

impl HookMatcher {
    /// Create a matcher that matches all tools
    pub fn all() -> Self {
        Self { tool_name: None }
    }

    /// Create a matcher for a specific tool name
    pub fn tool(name: impl Into<String>) -> Self {
        Self {
            tool_name: Some(name.into()),
        }
    }

    /// Check if this matcher matches the given tool name
    pub fn matches(&self, tool_name: &str) -> bool {
        match &self.tool_name {
            None => true, // Match all
            Some(pattern) => {
                // Exact match for now. TODO: Add wildcard support (mcp__*)
                pattern == tool_name
            }
        }
    }
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

/// Sandbox settings (placeholder for future sandbox tasks)
#[derive(Debug, Clone)]
pub struct SandboxSettings;

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
#[derive(Debug, Clone, Default)]
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
    /// Tools that require permission prompts
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
    pub settings_sources: Option<Vec<String>>,

    // Output
    /// Output format specification
    pub output_format: Option<serde_json::Value>,
    /// Include partial messages in output
    pub include_partial_messages: bool,

    // Advanced
    /// Beta features to enable
    pub betas: Vec<String>,
    /// Sandbox settings
    pub sandbox_settings: Option<SandboxSettings>,
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

    /// Convert options to Claude CLI arguments
    ///
    /// This method generates CLI arguments compatible with the Claude CLI,
    /// following SPEC.md section 2.2.
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
    /// assert!(args.contains(&"--max-turns=5".to_string()));
    /// ```
    pub fn to_cli_args(&self, prompt: &str) -> Vec<String> {
        let mut args = vec![
            "--output-format=stream-json".to_string(),
            "--verbose".to_string(),
        ];

        // Max turns
        if let Some(max_turns) = self.max_turns {
            args.push(format!("--max-turns={}", max_turns));
        }

        // Model
        if let Some(model) = &self.model {
            args.push(format!("--model={}", model));
        }

        // Permission mode
        if let Some(mode) = &self.permission_mode {
            args.push(format!("--permission-mode={}", mode.to_cli_arg()));
        }

        // System prompt
        if let Some(sys_prompt) = &self.system_prompt {
            match sys_prompt {
                SystemPrompt::Custom(text) => {
                    args.push("--system-prompt".to_string());
                    args.push(text.clone());
                }
                SystemPrompt::Preset { preset } => {
                    args.push(format!("--system-prompt-preset={}", preset));
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
            args.push(format!("--allowed-tools={}", self.allowed_tools.join(",")));
        }

        // Disallowed tools
        if !self.disallowed_tools.is_empty() {
            args.push(format!("--disallowed-tools={}", self.disallowed_tools.join(",")));
        }

        // Session options
        if let Some(resume) = &self.resume {
            args.push(format!("--resume={}", resume));
        }

        if self.fork_session {
            args.push("--fork-session".to_string());
        }

        if let Some(name) = &self.session_name {
            args.push(format!("--session-name={}", name));
        }

        if self.enable_file_checkpointing {
            args.push("--enable-file-checkpointing".to_string());
        }

        // Settings isolation for reproducibility
        match &self.settings_sources {
            Some(sources) => args.push(format!("--settings-sources={}", sources.join(","))),
            None => args.push("--settings-sources=".to_string()),
        }

        // Enable control protocol input
        args.push("--input-format=stream-json".to_string());

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
    pub fn settings_sources(mut self, sources: Vec<String>) -> Self {
        self.inner.settings_sources = Some(sources);
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

    /// Set beta features
    pub fn betas(mut self, betas: Vec<String>) -> Self {
        self.inner.betas = betas;
        self
    }

    /// Set sandbox settings
    pub fn sandbox_settings(mut self, settings: SandboxSettings) -> Self {
        self.inner.sandbox_settings = Some(settings);
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
        assert!(!opts.enable_file_checkpointing);
        assert!(!opts.include_partial_messages);
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
        assert!(matches!(opts.permission_mode, Some(PermissionMode::AcceptEdits)));
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
            .fork_session(true)
            .session_name("test-session")
            .enable_file_checkpointing(true)
            .cwd("/tmp")
            .include_partial_messages(true)
            .betas(vec!["feature-1".to_string()])
            .build();

        assert!(opts.system_prompt.is_some());
        assert_eq!(opts.append_system_prompt, Some("append".to_string()));
        assert_eq!(opts.max_turns, Some(10));
        assert_eq!(opts.model, Some("claude-opus-4".to_string()));
        assert_eq!(opts.allowed_tools, vec!["Read".to_string()]);
        assert_eq!(opts.disallowed_tools, vec!["Bash".to_string()]);
        assert!(matches!(opts.permission_mode, Some(PermissionMode::Plan)));
        assert_eq!(opts.permission_prompt_tool_allowlist, vec!["Edit".to_string()]);
        assert_eq!(opts.resume, Some("session-123".to_string()));
        assert!(opts.fork_session);
        assert_eq!(opts.session_name, Some("test-session".to_string()));
        assert!(opts.enable_file_checkpointing);
        assert!(opts.cwd.is_some());
        assert!(opts.include_partial_messages);
        assert_eq!(opts.betas, vec!["feature-1".to_string()]);
    }

    #[test]
    fn test_to_cli_args_minimal() {
        let opts = ClaudeAgentOptions::default();
        let args = opts.to_cli_args("test prompt");

        assert!(args.contains(&"--output-format=stream-json".to_string()));
        assert!(args.contains(&"--verbose".to_string()));
        assert!(args.contains(&"--input-format=stream-json".to_string()));
        assert!(args.contains(&"--settings-sources=".to_string()));
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

        assert!(args.contains(&"--max-turns=10".to_string()));
        assert!(args.contains(&"--model=claude-opus-4".to_string()));
        assert!(args.contains(&"--permission-mode=plan".to_string()));
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
            .system_prompt(SystemPrompt::Preset { preset: "assistant".to_string() })
            .build();

        let args = opts.to_cli_args("test");

        assert!(args.contains(&"--system-prompt-preset=assistant".to_string()));
    }

    #[test]
    fn test_to_cli_args_allowed_tools() {
        let opts = ClaudeAgentOptions::builder()
            .allowed_tools(vec!["Read".to_string(), "Bash".to_string()])
            .build();

        let args = opts.to_cli_args("test");

        assert!(args.contains(&"--allowed-tools=Read,Bash".to_string()));
    }

    #[test]
    fn test_to_cli_args_disallowed_tools() {
        let opts = ClaudeAgentOptions::builder()
            .disallowed_tools(vec!["Edit".to_string(), "Write".to_string()])
            .build();

        let args = opts.to_cli_args("test");

        assert!(args.contains(&"--disallowed-tools=Edit,Write".to_string()));
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

        assert!(args.contains(&"--resume=session-123".to_string()));
        assert!(args.contains(&"--fork-session".to_string()));
        assert!(args.contains(&"--session-name=my-session".to_string()));
        assert!(args.contains(&"--enable-file-checkpointing".to_string()));
    }

    #[test]
    fn test_to_cli_args_settings_sources_default() {
        // When no settings_sources configured, should emit empty --settings-sources=
        let opts = ClaudeAgentOptions::default();
        let args = opts.to_cli_args("test");
        assert!(args.contains(&"--settings-sources=".to_string()));
    }

    #[test]
    fn test_to_cli_args_settings_sources_custom() {
        // When settings_sources configured, should use the user's values
        let opts = ClaudeAgentOptions::builder()
            .settings_sources(vec!["local".to_string(), "project".to_string()])
            .build();
        let args = opts.to_cli_args("test");
        assert!(args.contains(&"--settings-sources=local,project".to_string()));
        assert!(!args.contains(&"--settings-sources=".to_string()));
    }

    #[test]
    fn test_permission_mode_to_cli_arg() {
        assert_eq!(PermissionMode::Default.to_cli_arg(), "default");
        assert_eq!(PermissionMode::AcceptEdits.to_cli_arg(), "acceptEdits");
        assert_eq!(PermissionMode::BypassPermissions.to_cli_arg(), "bypassPermissions");
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
    }

    #[test]
    fn test_collections_handling() {
        let mut env = HashMap::new();
        env.insert("KEY".to_string(), "value".to_string());

        let opts = ClaudeAgentOptions::builder()
            .env(env.clone())
            .build();

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
}

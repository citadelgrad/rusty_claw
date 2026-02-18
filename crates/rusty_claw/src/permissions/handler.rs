//! Default permission handler implementation.

use crate::control::handlers::CanUseToolHandler;
use crate::error::ClawError;
use crate::options::PermissionMode;
use async_trait::async_trait;
use serde_json::Value;

/// Default implementation of tool permission checking.
///
/// This handler evaluates tool usage requests through multiple layers:
///
/// 1. **Explicit Deny** - Check disallowed_tools first (highest priority)
/// 2. **Explicit Allow** - Check allowed_tools second
/// 3. **Default Policy** - Fall back to PermissionMode setting
///
/// Future enhancement: Hook integration for custom permission logic.
///
/// # Examples
///
/// ```rust
/// use rusty_claw::permissions::DefaultPermissionHandler;
/// use rusty_claw::options::PermissionMode;
///
/// // Allow only specific tools
/// let handler = DefaultPermissionHandler::builder()
///     .mode(PermissionMode::Deny)
///     .allowed_tools(vec!["bash".to_string(), "read".to_string()])
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct DefaultPermissionHandler {
    mode: PermissionMode,
    allowed_tools: Vec<String>,
    disallowed_tools: Vec<String>,
}

impl DefaultPermissionHandler {
    /// Create a new builder for configuring permission handler.
    pub fn builder() -> DefaultPermissionHandlerBuilder {
        DefaultPermissionHandlerBuilder::default()
    }

    /// Check if a tool is explicitly allowed.
    fn is_allowed(&self, tool_name: &str) -> bool {
        self.allowed_tools.is_empty() || self.allowed_tools.contains(&tool_name.to_string())
    }

    /// Check if a tool is explicitly denied.
    fn is_denied(&self, tool_name: &str) -> bool {
        self.disallowed_tools.contains(&tool_name.to_string())
    }

    /// Evaluate default policy based on PermissionMode.
    fn default_policy(&self) -> bool {
        match self.mode {
            PermissionMode::Allow => true,
            PermissionMode::Deny => false,
            PermissionMode::Ask => false, // Default to deny, CLI should prompt
            PermissionMode::Custom => false, // Require hook, deny if no hook
            // Legacy modes default to allow for backward compatibility
            PermissionMode::Default
            | PermissionMode::AcceptEdits
            | PermissionMode::BypassPermissions
            | PermissionMode::Plan => true,
        }
    }
}

#[async_trait]
impl CanUseToolHandler for DefaultPermissionHandler {
    async fn can_use_tool(&self, tool_name: &str, _tool_input: &Value) -> Result<bool, ClawError> {
        // 1. Check explicit deny list first (highest priority)
        if self.is_denied(tool_name) {
            return Ok(false);
        }

        // 2. Check explicit allow list
        if !self.allowed_tools.is_empty() && self.is_allowed(tool_name) {
            return Ok(true);
        }

        // 3. Check if allowed_tools is empty (no restrictions)
        if self.allowed_tools.is_empty() && !self.is_denied(tool_name) {
            // Fall through to default policy
        } else if !self.allowed_tools.is_empty() && !self.is_allowed(tool_name) {
            // Tool not in allowlist and allowlist is not empty
            return Ok(self.default_policy());
        }

        // 4. Fall back to default policy
        Ok(self.default_policy())
    }
}

/// Builder for [`DefaultPermissionHandler`].
///
/// # Examples
///
/// ```rust
/// use rusty_claw::permissions::DefaultPermissionHandler;
/// use rusty_claw::options::PermissionMode;
///
/// let handler = DefaultPermissionHandler::builder()
///     .mode(PermissionMode::Ask)
///     .allowed_tools(vec!["bash".to_string()])
///     .disallowed_tools(vec!["write".to_string()])
///     .build();
/// ```
#[derive(Debug, Default)]
pub struct DefaultPermissionHandlerBuilder {
    mode: Option<PermissionMode>,
    allowed_tools: Vec<String>,
    disallowed_tools: Vec<String>,
}

impl DefaultPermissionHandlerBuilder {
    /// Set the permission mode.
    pub fn mode(mut self, mode: PermissionMode) -> Self {
        self.mode = Some(mode);
        self
    }

    /// Set the list of allowed tools.
    pub fn allowed_tools(mut self, tools: Vec<String>) -> Self {
        self.allowed_tools = tools;
        self
    }

    /// Set the list of disallowed tools.
    pub fn disallowed_tools(mut self, tools: Vec<String>) -> Self {
        self.disallowed_tools = tools;
        self
    }

    /// Build the permission handler.
    pub fn build(self) -> DefaultPermissionHandler {
        DefaultPermissionHandler {
            mode: self.mode.unwrap_or(PermissionMode::Default),
            allowed_tools: self.allowed_tools,
            disallowed_tools: self.disallowed_tools,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_allow_mode_allows_all() {
        let handler = DefaultPermissionHandler::builder()
            .mode(PermissionMode::Allow)
            .build();

        assert!(handler.can_use_tool("bash", &Value::Null).await.unwrap());
        assert!(handler.can_use_tool("read", &Value::Null).await.unwrap());
        assert!(handler.can_use_tool("write", &Value::Null).await.unwrap());
    }

    #[tokio::test]
    async fn test_deny_mode_denies_all() {
        let handler = DefaultPermissionHandler::builder()
            .mode(PermissionMode::Deny)
            .build();

        assert!(!handler.can_use_tool("bash", &Value::Null).await.unwrap());
        assert!(!handler.can_use_tool("read", &Value::Null).await.unwrap());
        assert!(!handler.can_use_tool("write", &Value::Null).await.unwrap());
    }

    #[tokio::test]
    async fn test_explicit_allow_overrides_deny_mode() {
        let handler = DefaultPermissionHandler::builder()
            .mode(PermissionMode::Deny)
            .allowed_tools(vec!["bash".to_string(), "read".to_string()])
            .build();

        assert!(handler.can_use_tool("bash", &Value::Null).await.unwrap());
        assert!(handler.can_use_tool("read", &Value::Null).await.unwrap());
        assert!(!handler.can_use_tool("write", &Value::Null).await.unwrap());
    }

    #[tokio::test]
    async fn test_explicit_deny_overrides_allow_mode() {
        let handler = DefaultPermissionHandler::builder()
            .mode(PermissionMode::Allow)
            .disallowed_tools(vec!["bash".to_string(), "write".to_string()])
            .build();

        assert!(!handler.can_use_tool("bash", &Value::Null).await.unwrap());
        assert!(handler.can_use_tool("read", &Value::Null).await.unwrap());
        assert!(!handler.can_use_tool("write", &Value::Null).await.unwrap());
    }

    #[tokio::test]
    async fn test_explicit_deny_beats_explicit_allow() {
        let handler = DefaultPermissionHandler::builder()
            .mode(PermissionMode::Allow)
            .allowed_tools(vec!["bash".to_string()])
            .disallowed_tools(vec!["bash".to_string()])
            .build();

        assert!(!handler.can_use_tool("bash", &Value::Null).await.unwrap());
    }

    #[tokio::test]
    async fn test_ask_mode_defaults_to_deny() {
        let handler = DefaultPermissionHandler::builder()
            .mode(PermissionMode::Ask)
            .build();

        // Ask mode should default to deny, expecting CLI to prompt
        assert!(!handler.can_use_tool("bash", &Value::Null).await.unwrap());
    }

    #[tokio::test]
    async fn test_custom_mode_defaults_to_deny() {
        let handler = DefaultPermissionHandler::builder()
            .mode(PermissionMode::Custom)
            .build();

        // Custom mode should default to deny, expecting hooks
        assert!(!handler.can_use_tool("bash", &Value::Null).await.unwrap());
    }

    #[tokio::test]
    async fn test_legacy_mode_defaults_to_allow() {
        let handler = DefaultPermissionHandler::builder()
            .mode(PermissionMode::Default)
            .build();

        assert!(handler.can_use_tool("bash", &Value::Null).await.unwrap());

        let handler = DefaultPermissionHandler::builder()
            .mode(PermissionMode::AcceptEdits)
            .build();

        assert!(handler.can_use_tool("bash", &Value::Null).await.unwrap());
    }

    #[tokio::test]
    async fn test_empty_lists_uses_default_policy() {
        let handler = DefaultPermissionHandler::builder()
            .mode(PermissionMode::Allow)
            .allowed_tools(vec![])
            .disallowed_tools(vec![])
            .build();

        assert!(handler.can_use_tool("bash", &Value::Null).await.unwrap());

        let handler = DefaultPermissionHandler::builder()
            .mode(PermissionMode::Deny)
            .allowed_tools(vec![])
            .disallowed_tools(vec![])
            .build();

        assert!(!handler.can_use_tool("bash", &Value::Null).await.unwrap());
    }

    #[tokio::test]
    async fn test_allowlist_restricts_when_not_empty() {
        let handler = DefaultPermissionHandler::builder()
            .mode(PermissionMode::Allow)
            .allowed_tools(vec!["bash".to_string()])
            .build();

        assert!(handler.can_use_tool("bash", &Value::Null).await.unwrap());
        // Tools not in allowlist should follow default policy
        assert!(handler.can_use_tool("read", &Value::Null).await.unwrap());
    }

    #[tokio::test]
    async fn test_builder_defaults() {
        let handler = DefaultPermissionHandler::builder().build();

        // Should use Default mode with empty lists
        assert!(handler.can_use_tool("bash", &Value::Null).await.unwrap());
    }

    #[tokio::test]
    async fn test_bypass_permissions_mode() {
        let handler = DefaultPermissionHandler::builder()
            .mode(PermissionMode::BypassPermissions)
            .build();

        // Legacy modes should allow all tools
        assert!(handler.can_use_tool("bash", &Value::Null).await.unwrap());
        assert!(handler.can_use_tool("write", &Value::Null).await.unwrap());
    }

    #[tokio::test]
    async fn test_plan_mode() {
        let handler = DefaultPermissionHandler::builder()
            .mode(PermissionMode::Plan)
            .build();

        // Plan mode should allow all tools (legacy behavior)
        assert!(handler.can_use_tool("bash", &Value::Null).await.unwrap());
    }

    #[tokio::test]
    async fn test_complex_allowlist_denylist() {
        let handler = DefaultPermissionHandler::builder()
            .mode(PermissionMode::Ask)
            .allowed_tools(vec![
                "bash".to_string(),
                "read".to_string(),
                "write".to_string(),
            ])
            .disallowed_tools(vec!["write".to_string()])
            .build();

        // bash and read are in allowlist
        assert!(handler.can_use_tool("bash", &Value::Null).await.unwrap());
        assert!(handler.can_use_tool("read", &Value::Null).await.unwrap());

        // write is in both allowlist and denylist - deny wins
        assert!(!handler.can_use_tool("write", &Value::Null).await.unwrap());

        // grep not in allowlist, but not denied - follows default policy
        assert!(!handler.can_use_tool("grep", &Value::Null).await.unwrap());
    }

    #[tokio::test]
    async fn test_tool_input_parameter_ignored() {
        let handler = DefaultPermissionHandler::builder()
            .mode(PermissionMode::Allow)
            .build();

        // Should ignore tool_input parameter (for now)
        let complex_input = serde_json::json!({
            "command": "rm -rf /",
            "dangerous": true
        });

        assert!(handler.can_use_tool("bash", &complex_input).await.unwrap());
    }

    // Integration scenarios
    #[tokio::test]
    async fn test_realistic_read_only_policy() {
        // Scenario: Agent that can only read, not write
        let handler = DefaultPermissionHandler::builder()
            .mode(PermissionMode::Deny)
            .allowed_tools(vec![
                "read".to_string(),
                "glob".to_string(),
                "grep".to_string(),
            ])
            .build();

        // Read operations allowed
        assert!(handler.can_use_tool("read", &json!({})).await.unwrap());
        assert!(handler.can_use_tool("glob", &json!({})).await.unwrap());
        assert!(handler.can_use_tool("grep", &json!({})).await.unwrap());

        // Write operations denied
        assert!(!handler.can_use_tool("write", &json!({})).await.unwrap());
        assert!(!handler.can_use_tool("edit", &json!({})).await.unwrap());
        assert!(!handler.can_use_tool("bash", &json!({})).await.unwrap());
    }

    #[tokio::test]
    async fn test_safe_tools_policy() {
        // Scenario: Allow all except dangerous tools
        let handler = DefaultPermissionHandler::builder()
            .mode(PermissionMode::Allow)
            .disallowed_tools(vec![
                "bash".to_string(),
                "write".to_string(),
                "delete".to_string(),
            ])
            .build();

        // Safe tools allowed
        assert!(handler.can_use_tool("read", &json!({})).await.unwrap());
        assert!(handler.can_use_tool("grep", &json!({})).await.unwrap());

        // Dangerous tools denied
        assert!(!handler.can_use_tool("bash", &json!({})).await.unwrap());
        assert!(!handler.can_use_tool("write", &json!({})).await.unwrap());
        assert!(!handler.can_use_tool("delete", &json!({})).await.unwrap());
    }

    #[tokio::test]
    async fn test_can_use_tool_trait() {
        // Verify that DefaultPermissionHandler properly implements CanUseToolHandler
        let handler: Box<dyn CanUseToolHandler> = Box::new(
            DefaultPermissionHandler::builder()
                .mode(PermissionMode::Allow)
                .build(),
        );

        let result = handler.can_use_tool("bash", &json!({})).await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }
}

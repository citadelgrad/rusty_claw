//! Hook response types for permission decisions and context injection.

use serde::Serialize;
use serde_json::Value;

/// Permission decision for tool use or other controlled actions
///
/// # Examples
///
/// ```
/// use rusty_claw::prelude::*;
///
/// let decision = PermissionDecision::Allow;
/// assert_eq!(serde_json::to_string(&decision).unwrap(), r#""allow""#);
/// ```
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionDecision {
    /// Allow the action to proceed
    Allow,
    /// Deny the action
    Deny,
    /// Ask the user for permission
    Ask,
}

/// Helper function for default true value
#[allow(dead_code)]
fn default_true() -> bool {
    true
}

/// Response from a hook callback
///
/// # Examples
///
/// ```
/// use rusty_claw::prelude::*;
///
/// // Allow with reason
/// let response = HookResponse::allow("Safe operation");
///
/// // Deny with reason
/// let response = HookResponse::deny("Dangerous operation detected");
///
/// // Ask user
/// let response = HookResponse::ask("Confirm destructive operation?");
///
/// // Custom response with additional context
/// let response = HookResponse::default()
///     .with_permission(PermissionDecision::Allow)
///     .with_context("Additional context for Claude");
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct HookResponse {
    /// Permission decision (Allow/Deny/Ask)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_decision: Option<PermissionDecision>,

    /// Reason for the permission decision (shown to user)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_decision_reason: Option<String>,

    /// Additional context to inject into Claude's prompt
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_context: Option<String>,

    /// Whether to continue processing subsequent hooks
    #[serde(rename = "continue", default = "default_true")]
    pub should_continue: bool,

    /// Modified tool input (if tool input should be transformed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_input: Option<Value>,
}

impl Default for HookResponse {
    fn default() -> Self {
        Self {
            permission_decision: None,
            permission_decision_reason: None,
            additional_context: None,
            should_continue: true, // Default to true
            updated_input: None,
        }
    }
}

impl HookResponse {
    /// Create a response that allows the action
    pub fn allow(reason: impl Into<String>) -> Self {
        Self {
            permission_decision: Some(PermissionDecision::Allow),
            permission_decision_reason: Some(reason.into()),
            ..Default::default()
        }
    }

    /// Create a response that denies the action
    pub fn deny(reason: impl Into<String>) -> Self {
        Self {
            permission_decision: Some(PermissionDecision::Deny),
            permission_decision_reason: Some(reason.into()),
            should_continue: false,
            ..Default::default()
        }
    }

    /// Create a response that asks the user
    pub fn ask(prompt: impl Into<String>) -> Self {
        Self {
            permission_decision: Some(PermissionDecision::Ask),
            permission_decision_reason: Some(prompt.into()),
            ..Default::default()
        }
    }

    /// Set permission decision
    pub fn with_permission(mut self, decision: PermissionDecision) -> Self {
        self.permission_decision = Some(decision);
        self
    }

    /// Set permission reason
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.permission_decision_reason = Some(reason.into());
        self
    }

    /// Add additional context for Claude
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.additional_context = Some(context.into());
        self
    }

    /// Set whether to continue processing hooks
    pub fn with_continue(mut self, should_continue: bool) -> Self {
        self.should_continue = should_continue;
        self
    }

    /// Set updated tool input
    pub fn with_updated_input(mut self, input: Value) -> Self {
        self.updated_input = Some(input);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_permission_decision_serialization() {
        assert_eq!(
            serde_json::to_string(&PermissionDecision::Allow).unwrap(),
            r#""allow""#
        );
        assert_eq!(
            serde_json::to_string(&PermissionDecision::Deny).unwrap(),
            r#""deny""#
        );
        assert_eq!(
            serde_json::to_string(&PermissionDecision::Ask).unwrap(),
            r#""ask""#
        );
    }

    #[test]
    fn test_hook_response_allow() {
        let response = HookResponse::allow("Safe operation");
        assert!(matches!(
            response.permission_decision,
            Some(PermissionDecision::Allow)
        ));
        assert_eq!(
            response.permission_decision_reason,
            Some("Safe operation".to_string())
        );
        assert!(response.should_continue);
    }

    #[test]
    fn test_hook_response_deny() {
        let response = HookResponse::deny("Dangerous operation");
        assert!(matches!(
            response.permission_decision,
            Some(PermissionDecision::Deny)
        ));
        assert_eq!(
            response.permission_decision_reason,
            Some("Dangerous operation".to_string())
        );
        assert!(!response.should_continue);
    }

    #[test]
    fn test_hook_response_ask() {
        let response = HookResponse::ask("Confirm?");
        assert!(matches!(
            response.permission_decision,
            Some(PermissionDecision::Ask)
        ));
        assert_eq!(
            response.permission_decision_reason,
            Some("Confirm?".to_string())
        );
    }

    #[test]
    fn test_hook_response_builder() {
        let response = HookResponse::default()
            .with_permission(PermissionDecision::Allow)
            .with_reason("test reason")
            .with_context("test context")
            .with_continue(false)
            .with_updated_input(json!({"key": "value"}));

        assert!(matches!(
            response.permission_decision,
            Some(PermissionDecision::Allow)
        ));
        assert_eq!(
            response.permission_decision_reason,
            Some("test reason".to_string())
        );
        assert_eq!(
            response.additional_context,
            Some("test context".to_string())
        );
        assert!(!response.should_continue);
        assert!(response.updated_input.is_some());
    }

    #[test]
    fn test_hook_response_serialization() {
        let response = HookResponse::allow("test");
        let json = serde_json::to_value(&response).unwrap();

        assert_eq!(json["permission_decision"], "allow");
        assert_eq!(json["permission_decision_reason"], "test");
        assert_eq!(json["continue"], true);
    }

    #[test]
    fn test_hook_response_default_serialization() {
        let response = HookResponse::default();
        let json = serde_json::to_value(&response).unwrap();

        // Default should serialize to minimal JSON
        assert!(!json.as_object().unwrap().contains_key("permission_decision"));
        assert!(!json
            .as_object()
            .unwrap()
            .contains_key("permission_decision_reason"));
        assert_eq!(json["continue"], true);
    }
}

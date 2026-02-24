//! Hook response types for permission decisions and context injection.

use serde::Serialize;
use serde_json::Value;

/// Permission decision for tool use or other controlled actions.
///
/// Used within [`HookResponse`] to signal allow/deny/ask to the CLI.
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

/// Typed hook output variants for specific response capabilities.
///
/// Each variant represents a different kind of hook response, providing
/// richer control than a plain allow/deny decision:
///
/// - [`HookOutput::Approve`]: Allow the action, with an optional message.
/// - [`HookOutput::Deny`]: Block the action with a reason.
/// - [`HookOutput::InjectSystemMessage`]: Inject a system message into the conversation.
/// - [`HookOutput::Stop`]: Stop the agent session.
/// - [`HookOutput::SuppressOutput`]: Suppress the tool output from the transcript.
/// - [`HookOutput::ModifyOutput`]: Replace tool output (for PostToolUse hooks).
///
/// # Examples
///
/// ```
/// use rusty_claw::hooks::HookOutput;
///
/// // Inject safety context
/// let out = HookOutput::InjectSystemMessage {
///     content: "Warning: destructive tool detected.".to_string(),
/// };
///
/// // Block a dangerous operation
/// let out = HookOutput::Deny {
///     reason: "Blocked for safety reasons".to_string(),
/// };
///
/// // Suppress verbose tool output
/// let out = HookOutput::SuppressOutput;
/// ```
#[derive(Debug, Clone)]
pub enum HookOutput {
    /// Allow the action to proceed.
    Approve {
        /// Optional message explaining the approval (shown in logs).
        message: Option<String>,
    },

    /// Deny the action.
    Deny {
        /// Reason for the denial (shown to the user).
        reason: String,
    },

    /// Inject a system message into the conversation context.
    ///
    /// The injected message is added as a system-level message before the
    /// next model turn. Useful for adding safety warnings or audit context.
    InjectSystemMessage {
        /// The system message content to inject.
        content: String,
    },

    /// Stop the agent session.
    Stop {
        /// Reason for stopping (used in the session transcript).
        reason: String,
    },

    /// Suppress the tool output from the conversation transcript.
    ///
    /// The tool still runs, but its output is not shown to the model.
    SuppressOutput,

    /// Replace the tool's output with a modified version.
    ///
    /// Available for `PostToolUse` hooks. The provided value replaces
    /// the tool's actual output before it is sent to the model.
    ModifyOutput {
        /// The replacement tool output value.
        updated_output: Value,
    },
}

impl HookOutput {
    /// Convert to a [`HookResponse`] for use in the control protocol.
    pub fn into_response(self) -> HookResponse {
        match self {
            HookOutput::Approve { message } => HookResponse {
                permission_decision: Some(PermissionDecision::Allow),
                permission_decision_reason: message,
                ..Default::default()
            },
            HookOutput::Deny { reason } => HookResponse {
                permission_decision: Some(PermissionDecision::Deny),
                permission_decision_reason: Some(reason),
                should_continue: false,
                ..Default::default()
            },
            HookOutput::InjectSystemMessage { content } => HookResponse {
                additional_context: Some(content),
                ..Default::default()
            },
            HookOutput::Stop { reason } => HookResponse {
                permission_decision: Some(PermissionDecision::Deny),
                permission_decision_reason: Some(reason),
                should_continue: false,
                ..Default::default()
            },
            HookOutput::SuppressOutput => HookResponse {
                suppress_output: true,
                ..Default::default()
            },
            HookOutput::ModifyOutput { updated_output } => HookResponse {
                updated_input: Some(updated_output),
                ..Default::default()
            },
        }
    }
}

/// Response from a hook callback.
///
/// For richer, typed responses consider constructing a [`HookOutput`] and
/// calling [`.into_response()`](HookOutput::into_response).
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

    /// Modified tool output (for PostToolUse hooks)
    ///
    /// When set, the tool's actual output is replaced with this value
    /// before being sent to the model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_output: Option<Value>,

    /// Stop reason (when the hook signals session stop)
    ///
    /// When set, the agent session is stopped after the hook completes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,

    /// System message to inject into the conversation
    ///
    /// When set, this string is injected as a system-level message before
    /// the next model turn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_message: Option<String>,

    /// Whether to suppress tool output from the transcript
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub suppress_output: bool,
}

impl Default for HookResponse {
    fn default() -> Self {
        Self {
            permission_decision: None,
            permission_decision_reason: None,
            additional_context: None,
            should_continue: true,
            updated_input: None,
            updated_output: None,
            stop_reason: None,
            system_message: None,
            suppress_output: false,
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

    /// Set updated tool output (for PostToolUse hooks)
    ///
    /// The provided value replaces the tool's actual output before it
    /// is sent to the model.
    pub fn with_updated_output(mut self, output: Value) -> Self {
        self.updated_output = Some(output);
        self
    }

    /// Set a stop reason (signals the agent to stop)
    pub fn with_stop_reason(mut self, reason: impl Into<String>) -> Self {
        self.stop_reason = Some(reason.into());
        self
    }

    /// Set a system message to inject into the conversation
    pub fn with_system_message(mut self, message: impl Into<String>) -> Self {
        self.system_message = Some(message.into());
        self
    }

    /// Suppress tool output from the transcript
    pub fn with_suppress_output(mut self, suppress: bool) -> Self {
        self.suppress_output = suppress;
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

        assert!(!json.as_object().unwrap().contains_key("permission_decision"));
        assert!(!json.as_object().unwrap().contains_key("permission_decision_reason"));
        assert_eq!(json["continue"], true);
        assert!(!json.as_object().unwrap().contains_key("suppress_output"));
    }

    #[test]
    fn test_hook_output_approve() {
        let out = HookOutput::Approve {
            message: Some("Safe".to_string()),
        };
        let resp = out.into_response();
        assert!(matches!(
            resp.permission_decision,
            Some(PermissionDecision::Allow)
        ));
        assert_eq!(resp.permission_decision_reason, Some("Safe".to_string()));
    }

    #[test]
    fn test_hook_output_deny() {
        let out = HookOutput::Deny {
            reason: "Too dangerous".to_string(),
        };
        let resp = out.into_response();
        assert!(matches!(
            resp.permission_decision,
            Some(PermissionDecision::Deny)
        ));
        assert!(!resp.should_continue);
    }

    #[test]
    fn test_hook_output_inject_system_message() {
        let out = HookOutput::InjectSystemMessage {
            content: "Safety warning injected".to_string(),
        };
        let resp = out.into_response();
        assert_eq!(
            resp.additional_context,
            Some("Safety warning injected".to_string())
        );
    }

    #[test]
    fn test_hook_output_stop() {
        let out = HookOutput::Stop {
            reason: "User requested stop".to_string(),
        };
        let resp = out.into_response();
        assert!(matches!(
            resp.permission_decision,
            Some(PermissionDecision::Deny)
        ));
        assert!(!resp.should_continue);
    }

    #[test]
    fn test_hook_output_suppress_output() {
        let out = HookOutput::SuppressOutput;
        let resp = out.into_response();
        assert!(resp.suppress_output);
    }

    #[test]
    fn test_hook_output_modify_output() {
        let out = HookOutput::ModifyOutput {
            updated_output: json!({"sanitized": true}),
        };
        let resp = out.into_response();
        assert!(resp.updated_input.is_some());
        assert_eq!(resp.updated_input.unwrap()["sanitized"], true);
    }

    #[test]
    fn test_hook_response_updated_output() {
        let response = HookResponse::default()
            .with_updated_output(json!({"result": "sanitized"}));
        assert!(response.updated_output.is_some());
        assert_eq!(response.updated_output.unwrap()["result"], "sanitized");
    }

    #[test]
    fn test_hook_response_stop_reason() {
        let response = HookResponse::default()
            .with_stop_reason("Detected dangerous pattern");
        assert_eq!(
            response.stop_reason,
            Some("Detected dangerous pattern".to_string())
        );
    }

    #[test]
    fn test_hook_response_system_message() {
        let response = HookResponse::default()
            .with_system_message("Additional safety context");
        assert_eq!(
            response.system_message,
            Some("Additional safety context".to_string())
        );
    }

    #[test]
    fn test_hook_response_suppress_output() {
        let response = HookResponse::default().with_suppress_output(true);
        assert!(response.suppress_output);

        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["suppress_output"], true);
    }

    #[test]
    fn test_hook_response_approve_without_message() {
        let out = HookOutput::Approve { message: None };
        let resp = out.into_response();
        assert!(matches!(
            resp.permission_decision,
            Some(PermissionDecision::Allow)
        ));
        assert!(resp.permission_decision_reason.is_none());
    }
}

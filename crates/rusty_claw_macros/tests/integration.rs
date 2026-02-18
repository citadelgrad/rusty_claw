//! Integration tests for the #[claw_tool] macro
//!
//! These tests verify that the macro correctly generates MCP tool definitions
//! from annotated functions.

use rusty_claw::mcp_server::{ToolContent, ToolResult};
use rusty_claw::prelude::*;
use rusty_claw_macros::claw_tool;
use serde_json::json;

/// Test basic tool with single String parameter
#[test]
fn test_basic_tool_with_name_and_description() {
    #[claw_tool(name = "echo", description = "Echo a message")]
    async fn echo_tool(message: String) -> ToolResult {
        ToolResult::text(message)
    }

    let tool = echo_tool();
    assert_eq!(tool.name, "echo");
    assert_eq!(tool.description, "Echo a message");

    // Verify schema structure
    let schema = &tool.input_schema;
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["properties"]["message"]["type"], "string");
    assert_eq!(schema["required"][0], "message");
}

/// Test tool without explicit name/description (inferred from function)
#[test]
fn test_tool_inferred_name() {
    #[claw_tool]
    async fn my_test_tool(value: String) -> ToolResult {
        ToolResult::text(value)
    }

    let tool = my_test_tool();
    assert_eq!(tool.name, "my-test-tool"); // Underscores converted to dashes
    assert!(tool.description.contains("my-test-tool"));
}

/// Test tool with doc comment as description
#[test]
fn test_tool_doc_comment() {
    /// This is a documented tool
    /// that spans multiple lines
    #[claw_tool]
    async fn documented_tool(input: String) -> ToolResult {
        ToolResult::text(input)
    }

    let tool = documented_tool();
    assert!(tool.description.contains("documented tool"));
}

/// Test tool with multiple parameters
#[test]
fn test_multiple_parameters() {
    #[claw_tool(name = "add", description = "Add two numbers")]
    async fn add_numbers(a: i32, b: i32) -> ToolResult {
        ToolResult::text(format!("{}", a + b))
    }

    let tool = add_numbers();
    assert_eq!(tool.name, "add");

    // Verify both parameters in schema
    let schema = &tool.input_schema;
    assert_eq!(schema["properties"]["a"]["type"], "number");
    assert_eq!(schema["properties"]["b"]["type"], "number");
    assert_eq!(schema["required"].as_array().unwrap().len(), 2);
}

/// Test tool with optional parameter
#[test]
fn test_optional_parameter() {
    #[claw_tool(name = "search", description = "Search with optional limit")]
    async fn search_tool(query: String, limit: Option<i32>) -> ToolResult {
        let max = limit.unwrap_or(10);
        ToolResult::text(format!("Search: {} (limit: {})", query, max))
    }

    let tool = search_tool();
    assert_eq!(tool.name, "search");

    // Verify query is required but limit is not
    let schema = &tool.input_schema;
    assert_eq!(schema["properties"]["query"]["type"], "string");
    assert_eq!(schema["properties"]["limit"]["type"], "number");

    let required = schema["required"].as_array().unwrap();
    assert_eq!(required.len(), 1);
    assert_eq!(required[0], "query");
}

/// Test tool with Vec parameter
#[test]
fn test_vec_parameter() {
    #[claw_tool(name = "sum", description = "Sum a list of numbers")]
    async fn sum_numbers(numbers: Vec<i32>) -> ToolResult {
        let total: i32 = numbers.iter().sum();
        ToolResult::text(format!("{}", total))
    }

    let tool = sum_numbers();
    assert_eq!(tool.name, "sum");

    // Verify array schema
    let schema = &tool.input_schema;
    assert_eq!(schema["properties"]["numbers"]["type"], "array");
    assert_eq!(schema["properties"]["numbers"]["items"]["type"], "number");
}

/// Test tool with boolean parameter
#[test]
fn test_boolean_parameter() {
    #[claw_tool(name = "toggle", description = "Toggle a setting")]
    async fn toggle_setting(enabled: bool) -> ToolResult {
        ToolResult::text(format!("Enabled: {}", enabled))
    }

    let tool = toggle_setting();
    let schema = &tool.input_schema;
    assert_eq!(schema["properties"]["enabled"]["type"], "boolean");
}

/// Test tool with various numeric types
#[test]
fn test_numeric_types() {
    #[claw_tool]
    async fn numeric_tool(i: i32, u: u32, f: f64) -> ToolResult {
        ToolResult::text(format!("{} {} {}", i, u, f))
    }

    let tool = numeric_tool();
    let schema = &tool.input_schema;
    assert_eq!(schema["properties"]["i"]["type"], "number");
    assert_eq!(schema["properties"]["u"]["type"], "number");
    assert_eq!(schema["properties"]["f"]["type"], "number");
}

/// Test tool execution with valid arguments
#[tokio::test]
async fn test_tool_execution() {
    #[claw_tool(name = "greet", description = "Greet someone")]
    async fn greet(name: String) -> ToolResult {
        ToolResult::text(format!("Hello, {}!", name))
    }

    let tool = greet();
    let result = tool.execute(json!({"name": "World"})).await.unwrap();

    assert_eq!(result.content.len(), 1);
    match &result.content[0] {
        ToolContent::Text { text } => assert_eq!(text, "Hello, World!"),
        _ => panic!("Expected text content"),
    }
}

/// Test tool execution with multiple parameters
#[tokio::test]
async fn test_tool_execution_multiple_params() {
    #[claw_tool(name = "calculate", description = "Perform calculation")]
    async fn calculate(x: i32, y: i32, op: String) -> ToolResult {
        let result = match op.as_str() {
            "add" => x + y,
            "sub" => x - y,
            "mul" => x * y,
            _ => 0,
        };
        ToolResult::text(format!("{}", result))
    }

    let tool = calculate();
    let result = tool
        .execute(json!({
            "x": 10,
            "y": 5,
            "op": "add"
        }))
        .await
        .unwrap();

    match &result.content[0] {
        ToolContent::Text { text } => assert_eq!(text, "15"),
        _ => panic!("Expected text content"),
    }
}

/// Test tool execution with optional parameter (provided)
#[tokio::test]
async fn test_tool_execution_optional_provided() {
    #[claw_tool]
    async fn search_with_limit(query: String, limit: Option<i32>) -> ToolResult {
        let max = limit.unwrap_or(10);
        ToolResult::text(format!("Query: {}, Limit: {}", query, max))
    }

    let tool = search_with_limit();
    let result = tool
        .execute(json!({
            "query": "test",
            "limit": 20
        }))
        .await
        .unwrap();

    match &result.content[0] {
        ToolContent::Text { text } => assert!(text.contains("Limit: 20")),
        _ => panic!("Expected text content"),
    }
}

/// Test tool execution with optional parameter (not provided)
#[tokio::test]
async fn test_tool_execution_optional_missing() {
    #[claw_tool]
    async fn search_without_limit(query: String, limit: Option<i32>) -> ToolResult {
        let max = limit.unwrap_or(10);
        ToolResult::text(format!("Query: {}, Limit: {}", query, max))
    }

    let tool = search_without_limit();
    let result = tool
        .execute(json!({
            "query": "test"
        }))
        .await
        .unwrap();

    match &result.content[0] {
        ToolContent::Text { text } => assert!(text.contains("Limit: 10")),
        _ => panic!("Expected text content"),
    }
}

/// Test tool execution with Vec parameter
#[tokio::test]
async fn test_tool_execution_vec() {
    #[claw_tool]
    async fn sum_array(numbers: Vec<i32>) -> ToolResult {
        let total: i32 = numbers.iter().sum();
        ToolResult::text(format!("{}", total))
    }

    let tool = sum_array();
    let result = tool
        .execute(json!({
            "numbers": [1, 2, 3, 4, 5]
        }))
        .await
        .unwrap();

    match &result.content[0] {
        ToolContent::Text { text } => assert_eq!(text, "15"),
        _ => panic!("Expected text content"),
    }
}

/// Test tool with complex optional Vec parameter
#[test]
fn test_optional_vec_parameter() {
    #[claw_tool]
    async fn filter_tool(query: String, tags: Option<Vec<String>>) -> ToolResult {
        let tag_list = tags
            .map(|t| t.join(", "))
            .unwrap_or_else(|| "none".to_string());
        ToolResult::text(format!("Query: {}, Tags: {}", query, tag_list))
    }

    let tool = filter_tool();
    let schema = &tool.input_schema;

    // Verify schema structure
    assert_eq!(schema["properties"]["query"]["type"], "string");
    assert_eq!(schema["properties"]["tags"]["type"], "array");
    assert_eq!(schema["properties"]["tags"]["items"]["type"], "string");

    // Only query should be required
    let required = schema["required"].as_array().unwrap();
    assert_eq!(required.len(), 1);
    assert_eq!(required[0], "query");
}

/// Test tool execution with Result return type
#[tokio::test]
async fn test_tool_with_result_return() {
    #[claw_tool]
    async fn fallible_tool(value: i32) -> Result<ToolResult, ClawError> {
        if value < 0 {
            Err(ClawError::ToolExecution(
                "Negative value not allowed".to_string(),
            ))
        } else {
            Ok(ToolResult::text(format!("Value: {}", value)))
        }
    }

    let tool = fallible_tool();

    // Test success case
    let result = tool.execute(json!({"value": 42})).await.unwrap();
    match &result.content[0] {
        ToolContent::Text { text } => assert_eq!(text, "Value: 42"),
        _ => panic!("Expected text content"),
    }

    // Test error case
    let error = tool.execute(json!({"value": -1})).await.unwrap_err();
    match error {
        ClawError::ToolExecution(msg) => {
            assert!(msg.contains("Negative value"));
        }
        _ => panic!("Expected ToolExecution error"),
    }
}

/// Test that generated tools implement Clone
#[test]
fn test_tool_is_clone() {
    #[claw_tool]
    async fn cloneable_tool(msg: String) -> ToolResult {
        ToolResult::text(msg)
    }

    let tool1 = cloneable_tool();
    let tool2 = tool1.clone();

    assert_eq!(tool1.name, tool2.name);
    assert_eq!(tool1.description, tool2.description);
}

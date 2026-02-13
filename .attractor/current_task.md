# Current Task: rusty_claw-bkm

## Task ID
rusty_claw-bkm

## Status
IN_PROGRESS

## Title
Write examples

## Priority
P3

## Description
Create examples: simple_query.rs, interactive_client.rs, custom_tool.rs, and hooks_guardrails.rs demonstrating core SDK usage patterns.

## Dependencies
- rusty_claw-qrl: Implement ClaudeClient for interactive sessions [CLOSED ✓]
- rusty_claw-tlh: Implement SDK MCP Server bridge [CLOSED ✓]

## Blocks
- rusty_claw-5uw: Documentation and crates.io prep [OPEN]

## Type
task

## Created
2026-02-12

## Updated
2026-02-13

## Acceptance Criteria

Create 4 working examples that demonstrate:
1. **simple_query.rs** - Basic SDK usage with simple queries
2. **interactive_client.rs** - Interactive multi-turn conversations using ClaudeClient
3. **custom_tool.rs** - Implementing and registering custom tools
4. **hooks_guardrails.rs** - Using hooks for guardrails and monitoring

Each example should:
- Be self-contained and runnable
- Include comprehensive comments
- Demonstrate best practices
- Compile without warnings
- Pass clippy linting

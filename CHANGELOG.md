# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-02-24

### Added

- **Core SDK** - `rusty_claw` crate with full Rust implementation of the Claude Agent SDK
- **Streaming query API** - `query()` and `ClaudeAgent::query()` for streaming Claude responses
- **Message types** - Strongly-typed `Message`, `AssistantMessage`, `ResultMessage`, `SystemMessage` enums
- **Agent options** - `ClaudeAgentOptions` builder for configuring allowed tools, permission mode, system prompt, MCP servers, and more
- **Permission system** - `DefaultPermissionHandler` with per-tool allow/deny lists and hook callbacks
- **MCP integration** - Register custom MCP tool servers via `register_mcp_message_handler()`; supports both stdio and SDK server types
- **Hook system** - `HookCallback` trait for intercepting pre-tool, post-tool, and stop events
- **Subagent support** - Spawn child agents and track their lifecycle
- **Session streaming** - `session_stream()` for long-running multi-turn interactions
- **CLI auto-discovery** - Automatically locates the Claude Code CLI binary
- **`#[claw_tool]` macro** - Derive macro for defining MCP tools with JSON schema generation
- **`rusty_claw_macros`** - Companion proc-macro crate
- **Parity with Python SDK** - Matches the `claude-agent-sdk` (Python) API surface: typed messages, MCP protocol v2025-11-25, control protocol, sdkMcpServers

[0.1.0]: https://github.com/citadelgrad/rusty_claw/releases/tag/v0.1.0

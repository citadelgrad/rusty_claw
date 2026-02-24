---
title: feat: Python SDK Parity - Complete Implementation with Swarm Agents
type: feat
date: 2026-02-24
epic: rusty_claw-elj
---

# Python SDK Parity — Swarm Implementation Plan

## Overview

This plan implements 26 features to bring `rusty_claw` to full parity with the Python Claude Agent SDK.
Work is organized into 5 parallel workstreams that can be executed concurrently, with internal
dependency chains handled within each workstream.

The gap analysis identified 35 gaps across 11 areas. The beads issues already contain full
specifications, background, and acceptance criteria — implementors should `bd show <id>` for details.

## Workstream Map

```
A: Multi-turn Client        rusty_claw-neo → rusty_claw-3mj
                                           → rusty_claw-klw

B: External MCP Servers     rusty_claw-bvo ┐
                            rusty_claw-9be ┼→ rusty_claw-xik → rusty_claw-yhn
                                           ┘

C: Options API              rusty_claw-21j ┐ (all independent)
                            rusty_claw-8vv │
                            rusty_claw-exy │
                            rusty_claw-ne5 │
                            rusty_claw-bks │
                            rusty_claw-eel ┘→ rusty_claw-f34

D: Permission + Hooks       rusty_claw-862 ┐→ rusty_claw-bme
                            rusty_claw-ng3 ┘
                            rusty_claw-d7g → rusty_claw-mfw
                            rusty_claw-jhq (independent)

E: Messages + MCP Ergonomics rusty_claw-i0f (independent)
                             rusty_claw-ahw (independent)
                             rusty_claw-6c4 (independent)
                             rusty_claw-abh (independent)
                             rusty_claw-ak2 (independent)
                             rusty_claw-om8 (independent)
```

## Workstream Details

### Workstream A: Multi-turn Client

**Goal:** Enable `ClaudeClient` to be used for multiple conversational turns, matching Python's `session.query()` pattern.

**Issues (ordered):**
1. `rusty_claw-neo` (P0) — Fix ClaudeClient multi-turn architecture
   - Current: `send_message()` consumes `message_rx` via `.take()`, making client single-use
   - Fix: Each `send_message()` call registers a new per-turn channel; messages route by turn ID
   - Key file: `crates/rusty_claw/src/client.rs`
2. `rusty_claw-3mj` (P1) — Add `receive_response()` method (depends on neo)
   - Separate sending from receiving; allow inspecting individual turn results
   - Key file: `crates/rusty_claw/src/client.rs`
3. `rusty_claw-klw` (P2) — Transport injection + `get_server_info()` + async RAII cleanup (depends on neo)
   - Allow passing custom transport for testing; add server info retrieval; drop connection on Client drop
   - Key files: `client.rs`, `transport/subprocess.rs`

**Test approach:** Integration tests in `tests/integration/` using a mock transport.

### Workstream B: External MCP Servers

**Goal:** Support connecting to external MCP servers (stdio, SSE, HTTP) via CLI `--mcp-config` flag.

**Issues (ordered):**
1. `rusty_claw-bvo` (P0) — McpStdioServerConfig: command, args, env fields
   - Define the struct; wire to `to_cli_args()` as `--mcp-config /tmp/mcp.json`
   - Key file: `crates/rusty_claw/src/options.rs`, `mcp_server.rs`
2. `rusty_claw-9be` (P0) — McpServerConfig enum variants (depends partly on bvo)
   - Enum: `Stdio(McpStdioServerConfig)`, `SSE(McpSSEServerConfig)`, `Http(McpHttpServerConfig)`
   - Wire to MCP config JSON generation in `to_cli_args()`
   - Key file: `crates/rusty_claw/src/options.rs`
3. `rusty_claw-xik` (P1) — McpSSEServerConfig + McpHttpServerConfig (depends on bvo)
   - Add `url`, `headers` fields; complete SSE/HTTP transport variants
4. `rusty_claw-yhn` (P0) — Wire `mcp_servers` HashMap to `--mcp-config` (depends on bvo, 9be, xik)
   - Serialize all configured servers to temp JSON file passed as `--mcp-config`
   - Key file: `crates/rusty_claw/src/options.rs` (`to_cli_args()`)

**Test approach:** Unit tests verifying JSON generation; integration test spawning a real stdio MCP server.

### Workstream C: Options API Completeness

**Goal:** Wire all currently-silent `ClaudeAgentOptions` fields to actual CLI args.

**Issues (all independent except eel→f34):**
- `rusty_claw-21j` (P1) — `continue_conversation`, `max_budget_usd`, `max_thinking_tokens`
  - Wire to `--continue`, `--max-budget-usd`, `--max-thinking-tokens` CLI args
- `rusty_claw-8vv` (P1) — `betas`, `output_format`, `sandbox_settings`, `permission_prompt_tool_allowlist`
  - Wire each to corresponding CLI args (see `to_cli_args()` in `options.rs`)
- `rusty_claw-exy` (P1) — `SandboxSettings` struct with all fields
  - Define complete struct matching Python SDK; wire to `--sandbox` CLI arg
- `rusty_claw-ne5` (P2) — Developer options: `add_dirs`, `user`, `extra_args`, `stderr_callback`, `fallback_model`, `max_buffer_size`
  - Add fields to `ClaudeAgentOptions`; wire appropriately
- `rusty_claw-bks` (P3) — `betas` Vec → `--beta` flag; `SdkBeta` typed constants
  - Add typed constants for known beta values: `SdkBeta::INTERLEAVED_THINKING`, etc.
- `rusty_claw-eel` (P1) — `output_format` → `--output-format json` for structured output
  - Wire format option; ensure structured JSON output schema works end-to-end
- `rusty_claw-f34` (P2) — Complete `ResultMessage` fields: `structured_output`, `is_error`, `duration_api_ms` (depends on eel)
  - Parse new fields from CLI JSON output

**Key file:** `crates/rusty_claw/src/options.rs` for most; `messages.rs` for f34.

### Workstream D: Permission System + Hooks

**Goal:** Rich permission results, ergonomic callback API, typed hook events.

**Issues:**
- `rusty_claw-862` (P1) — `CanUseToolHandler` returns `PermissionResult` (blocks bme)
  - Add `PermissionResult` enum: `Allow`, `Deny(reason)`, `AllowWithModifiedInput(Value)`
  - Update `DefaultPermissionHandler` to return new type
- `rusty_claw-ng3` (P1) — Enrich `CanUseToolHandler`: input mutation support (blocks bme)
  - Return modified tool input alongside decision
  - Key file: `crates/rusty_claw/src/permissions/`
- `rusty_claw-bme` (P1) — `can_use_tool` callback in `ClaudeAgentOptions` (depends on 862, ng3)
  - Add `permission_handler: Option<Box<dyn CanUseToolHandler>>` to options struct
  - Ergonomic builder method accepting async closure
- `rusty_claw-d7g` (P1) — Typed hook input variants (blocks mfw)
  - Replace generic `HookInput` with enum: `PreToolUse`, `PostToolUse`, `Stop`, `SubagentStop`, etc.
  - Key file: `crates/rusty_claw/src/hooks/`
- `rusty_claw-mfw` (P2) — Typed hook output variants (depends on d7g)
  - `HookResponse` with typed variants: `Approve`, `Deny`, `InjectSystemMessage`, `Stop`
- `rusty_claw-jhq` (P2) — `HookMatcher` enrichment: timeout, event array, wildcards
  - Add `timeout_ms`, `hooks` array field, `*` wildcard tool pattern

### Workstream E: Message Types + MCP Ergonomics

**Goal:** Complete message type coverage + improve MCP server API.

**Issues (all independent):**
- `rusty_claw-i0f` (P1) — `AssistantMessage.error` field with `AssistantMessageError` enum
  - Add typed error field parsed from `assistant` messages with error content
  - Key file: `crates/rusty_claw/src/messages.rs`
- `rusty_claw-ahw` (P2) — `ThinkingBlock.signature` field; `UserMessage.uuid` and `parent_tool_use_id`
  - Complete fields for richer message introspection
- `rusty_claw-6c4` (P2) — Type-safe `SdkMcpTool` handler input via serde
  - Tool handlers receive typed struct instead of raw `Value`; auto-derive from JSON schema
  - Key file: `crates/rusty_claw/src/mcp_server.rs`
- `rusty_claw-abh` (P2) — `create_sdk_mcp_server()` convenience function
  - One-call factory: `create_sdk_mcp_server("name", vec![tool1, tool2])`
- `rusty_claw-ak2` (P1) — Streaming input to `query()`: accept `Stream<Message>` for multi-turn
  - Allow injecting a message stream as input (not just a `&str` prompt)
- `rusty_claw-om8` (P3) — API naming alignment: `disconnect()` alias, `setting_sources` rename
  - Minor ergonomic fixes matching Python SDK naming conventions

## Agent Assignment (5+ parallel agents)

| Agent | Workstream | Issues | Priority |
|-------|-----------|--------|----------|
| Agent 1 | A: Multi-turn Client | neo → 3mj, klw | P0 critical |
| Agent 2 | B: External MCP Servers | bvo, 9be → xik → yhn | P0 critical |
| Agent 3 | C: Options API | 21j, 8vv, eel → f34, exy | P1 high |
| Agent 4 | C: Options (remaining) + E (parts) | ne5, bks, i0f, ahw, ak2 | P1-P2 |
| Agent 5 | D: Permission + Hooks | 862, ng3 → bme, d7g → mfw, jhq | P1 high |
| Agent 6 | E: Messages + MCP Ergonomics | 6c4, abh, om8 | P2-P3 |

## Key Conventions

### Code Style
- All new public APIs need `///` doc comments with examples
- Follow existing patterns in `options.rs`, `messages.rs`, `mcp_server.rs`
- Use `builder()` pattern for structs with >2 fields
- Errors go through `ClawError` enum in `error.rs`
- Async traits require `#[async_trait]`

### Testing
- Unit tests in same file under `#[cfg(test)]`
- Integration tests in `crates/rusty_claw/tests/`
- Compile-time trait tests (Send, Sync, Clone) are fine
- Run: `cargo test --workspace`
- Check: `cargo clippy --workspace --all-targets -- -D warnings`

### File Layout
```
crates/rusty_claw/src/
  client.rs          — ClaudeClient, send_message(), receive_response()
  options.rs         — ClaudeAgentOptions, to_cli_args(), all option structs
  messages.rs        — Message enum, all message type structs
  permissions/       — CanUseToolHandler, DefaultPermissionHandler
  hooks/             — HookCallback, HookInput, HookResponse, HookMatcher
  mcp_server.rs      — SdkMcpTool, SdkMcpServerRegistry, JSON-RPC handlers
  transport/         — SubprocessCLITransport, Transport trait
  control/           — ControlRequest/Response, pending requests
  error.rs           — ClawError enum
  query.rs           — query() one-shot function
```

### CLI Args Format
- Always space-separated: `"--flag".to_string(), value.to_string()`
- Never `=`-joined: ~~`"--flag=value".to_string()`~~
- See `options.rs:to_cli_args()` for examples

## Acceptance Criteria

- [ ] All 26 beads issues closed
- [ ] `cargo test --workspace` passes (currently 314 tests)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo doc --workspace --no-deps` produces no warnings
- [ ] `custom_tool` example still works end-to-end with MCP
- [ ] New examples added to `crates/rusty_claw/examples/` for each major feature
- [ ] All public APIs documented with doc comments and examples

## References

- Python SDK: `https://github.com/anthropics/claude-agent-sdk-python`
- Epic: `rusty_claw-elj`
- Previous deep code review: commit `e08ea4d` (4 bugs fixed)
- Key memory: `/Volumes/qwiizlab/projects/rusty_claw/memory/MEMORY.md`

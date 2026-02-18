# MCP Integration Troubleshooting Plan

## Status
All 5 planned code fixes are implemented and all tests pass. The blocking issue is that CLI v2.1.45 hangs (produces zero stdout) when `--mcp-config` is passed with an SDK-type server.

## Current Hypothesis
The CLI may not support `--mcp-config` with `"type":"sdk"` servers in v2.1.45, or it may require a specific `CLAUDE_CODE_ENTRYPOINT` value, or the SDK servers may need to be declared ONLY in the initialize control request (not via CLI flag).

## Next Investigation Steps

### 1. Check Python SDK's CLAUDE_CODE_ENTRYPOINT value
```bash
# Search the Python SDK source for ENTRYPOINT
curl -s https://raw.githubusercontent.com/anthropics/claude-agent-sdk-python/main/src/claude_agent_sdk/_internal/transport/subprocess_cli.py | grep -i entrypoint
```
We set `sdk-rust`. Python SDK likely sets something different.

### 2. Test WITHOUT --mcp-config flag (SDK servers in initialize only)
The Python SDK might only pass `--mcp-config` for EXTERNAL MCP servers (stdio/sse type), and SDK servers might only go in the initialize control request. Try removing `--mcp-config` and keeping `sdk_mcp_servers` only in the initialize request.

### 3. Check if --mcp-config exists in CLI v2.1.45
```bash
env -u CLAUDECODE claude --help 2>&1 | grep mcp
```

### 4. Test with Python SDK directly
```bash
uv run python -c "
from claude_code_sdk import query, ClaudeCodeOptions, SdkMcpServer
# minimal test to see if Python SDK has same hang
"
```

### 5. Check the Python SDK for whether --mcp-config is ONLY for external servers
Look at `_build_command()` more carefully - the filtering logic strips `"instance"` from SDK servers but still passes `{"type":"sdk"}`. But maybe the CLI doesn't support SDK type in --mcp-config and only uses it from the initialize request.

## Quick Test to Confirm Hypothesis #2
In `client.rs`, temporarily remove the `--mcp-config` push (the two cli_args.push lines) but keep `sdk_mcp_servers` in the initialize control request. If the CLI starts responding, hypothesis #2 is confirmed.

## Files to Edit
- `crates/rusty_claw/src/client.rs` lines ~336-347 (the --mcp-config block)

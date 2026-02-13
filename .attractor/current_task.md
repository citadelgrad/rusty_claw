# Current Task: rusty_claw-91n

## Task Information
- **ID:** rusty_claw-91n
- **Title:** Implement Control Protocol handler
- **Type:** task
- **Priority:** P1 (Critical)
- **Status:** in_progress
- **Owner:** Scott Nixon

## Description
Implement ControlProtocol struct with request/response routing, pending request tracking via oneshot channels, handler registration for can_use_tool/hook_callbacks/mcp_message, and the initialization handshake sequence.

## Dependencies (All Completed ✓)
- ✓ rusty_claw-6cn: Implement Transport trait and SubprocessCLITransport [P1]
- ✓ rusty_claw-dss: Implement ClaudeAgentOptions builder [P2]

## Blocks (Downstream Tasks)
- ○ rusty_claw-bip: Implement Hook system [P2]
- ○ rusty_claw-qrl: Implement ClaudeClient for interactive sessions [P2]
- ○ rusty_claw-tlh: Implement SDK MCP Server bridge [P2]

## Key Points
- Implement ControlProtocol struct with request/response routing
- Use oneshot channels for pending request tracking
- Register handlers for can_use_tool, hook_callbacks, and mcp_message events
- Implement initialization handshake sequence
- Production-ready with comprehensive tests

## Next Steps
1. Review Transport layer implementation (rusty_claw-6cn)
2. Review message types and specs (SPEC.md sections 3-5)
3. Design ControlProtocol struct with async handler system
4. Implement request/response routing
5. Implement oneshot channel tracking
6. Add initialization handshake
7. Write comprehensive unit tests
8. Document usage examples

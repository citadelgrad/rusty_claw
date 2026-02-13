# Current Task: rusty_claw-dss

## Task Information
- **ID:** rusty_claw-dss
- **Title:** Implement ClaudeAgentOptions builder
- **Type:** task
- **Priority:** P2 (Medium)
- **Status:** in_progress
- **Owner:** Scott Nixon

## Description
Implement ClaudeAgentOptions struct with builder pattern covering system_prompt, max_turns, model, allowed_tools, permission_mode, mcp_servers, hooks, agents, session, environment, and output settings.

## Dependencies
- ✓ **COMPLETED:** rusty_claw-pwc - Define shared types and message structs [P1]

## Blocks
- ○ **BLOCKED BY THIS TASK:** rusty_claw-91n - Implement Control Protocol handler [P1]

## Key Points
- Implement builder pattern for flexible configuration
- Cover all configuration options: system_prompt, max_turns, model, allowed_tools, permission_mode, mcp_servers, hooks, agents, session, environment, and output settings
- Enable downstream Control Protocol handler implementation
- Production-ready builder with sensible defaults

## Next Steps
1. Review existing types from rusty_claw-pwc
2. Design ClaudeAgentOptions struct with all required fields
3. Implement builder pattern
4. Add comprehensive tests
5. Document usage examples

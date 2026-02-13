# Current Task: rusty_claw-bip

## Task Information
- **ID:** rusty_claw-bip
- **Title:** Implement Hook system
- **Type:** task
- **Priority:** P2 (High)
- **Status:** in_progress
- **Owner:** Scott Nixon

## Description
Implement HookEvent enum, HookMatcher, HookCallback trait with blanket impl for closures, HookResponse with permission decisions, and hook invocation routing from control protocol callbacks.

## Dependencies (All Completed ✓)
- ✓ rusty_claw-91n: Implement Control Protocol handler [P1]

## Blocks (Downstream Tasks)
- ○ rusty_claw-s8q: Implement permission management [P2]

## Acceptance Criteria

1. **HookEvent enum** - Define events that can trigger hooks:
   - Tool use requests
   - Hook callbacks (from Claude CLI)
   - MCP message callbacks

2. **HookMatcher** - Pattern matching for hook triggers:
   - Match on event type
   - Match on specific event properties (tool name, hook type, etc.)
   - Support wildcards for flexible matching

3. **HookCallback trait** with blanket impl for closures:
   - Define async callback interface
   - Implement for `Fn` closure types
   - Return HookResponse with permission decisions

4. **HookResponse** with permission decisions:
   - Allow/Deny permission decisions
   - Optional message/reason
   - Optional metadata for upstream handlers

5. **Hook invocation routing** from control protocol callbacks:
   - Integrate with control protocol handler system
   - Route can_use_tool and hook_callbacks events to registered hooks
   - Return decisions back to Claude CLI

6. **Comprehensive tests:**
   - ~20 unit tests for hook matching and dispatch
   - Integration tests with control protocol
   - Zero clippy warnings in new code

7. **Complete documentation:**
   - Module-level docs with examples
   - Examples showing hook registration and responses

## Key Points
- Build on top of Control Protocol handler (just completed in rusty_claw-91n)
- Implement hook system for event-driven permission management
- Support flexible pattern matching with HookMatcher
- Integrate closures as callbacks with blanket trait impl
- Production-ready with comprehensive tests

## Next Steps
1. Investigate existing hook patterns and design
2. Implement HookEvent enum with all variants
3. Implement HookMatcher for pattern matching
4. Implement HookCallback trait with closures support
5. Implement HookResponse with permission decisions
6. Integrate hook invocation with control protocol
7. Write comprehensive tests (unit + integration)
8. Document with examples
9. Verify no regressions in existing tests

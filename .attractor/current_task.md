# Current Task: rusty_claw-s8q

## Task Information
- **ID:** rusty_claw-s8q
- **Title:** Implement permission management
- **Type:** task
- **Priority:** P2 (High)
- **Status:** in_progress
- **Owner:** Scott Nixon

## Description
Implement PermissionMode enum, permission prompt tool allowlist, and can_use_tool callback handler that routes permission checks through registered hooks or default policy.

## Dependencies (All Completed ✓)
- ✓ rusty_claw-bip: Implement Hook system [P2]

## Blocks (Downstream Tasks)
- ○ rusty_claw-isy: Add integration tests with mock CLI [P2]

## Acceptance Criteria

1. **PermissionMode enum** - Define permission check policies:
   - Allow (all tools allowed)
   - Ask (prompt for each tool use)
   - Deny (no tool use allowed)
   - Custom (custom permission logic)

2. **Tool allowlist** - Selective permission prompting:
   - Allowlist of tools that can be used without permission
   - Denylist of tools that require permission
   - Support for exact match and pattern-based filtering

3. **can_use_tool callback handler** - Route permission checks:
   - Integrate with Hook system for event routing
   - Check tool against allowlist/denylist
   - Invoke registered hooks for permission decisions

4. **Hook integration** - Permission decisions through hooks:
   - Route can_use_tool events to registered hooks
   - Support HookResponse permission decisions (Allow/Deny/Ask)
   - Fall back to default policy if no hooks registered

5. **Default permission policy** - Fallback behavior:
   - Implement default policy based on PermissionMode
   - Return Allow/Deny based on tool allowlist
   - Handle unknown tools appropriately

6. **Comprehensive tests:**
   - ~15-20 unit tests for permission checking
   - Integration tests with Hook system
   - Zero clippy warnings in new code

7. **Complete documentation:**
   - Module-level docs with examples
   - Examples showing permission configuration and hook integration

## Key Points
- Build on top of Hook system (just completed in rusty_claw-bip)
- Implement permission management for tool use control
- Support flexible tool allowlist/denylist configuration
- Route permission checks through Hook system
- Fall back to default policy if no hooks registered
- Production-ready with comprehensive tests

## Next Steps
1. Investigate existing permission patterns and architecture
2. Implement PermissionMode enum with policy options
3. Implement tool allowlist/denylist logic
4. Implement can_use_tool callback handler
5. Integrate with Hook system for event routing
6. Implement default permission policy
7. Write comprehensive tests (unit + integration with hooks)
8. Document with examples and use cases
9. Verify no regressions in existing tests

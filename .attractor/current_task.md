# Current Task: rusty_claw-pwc

## Define shared types and message structs

**Status:** IN_PROGRESS
**Priority:** P1
**Task ID:** rusty_claw-pwc

## Description

Implement Message enum with serde tagged variants (System, Assistant, User, Result), ContentBlock, SystemMessage, AssistantMessage, ResultMessage, StreamEvent, and supporting types like UsageInfo and ToolInfo.

## Key Types to Implement

1. **Message enum** - Serde tagged variants:
   - System
   - Assistant
   - User
   - Result

2. **ContentBlock** - Wrapper for message content

3. **Message Types**:
   - SystemMessage
   - AssistantMessage
   - ResultMessage

4. **Supporting Types**:
   - StreamEvent
   - UsageInfo
   - ToolInfo

## Blocks

- rusty_claw-sna: Implement query() function
- rusty_claw-1ke: Add unit tests for message parsing and fixtures
- rusty_claw-dss: Implement ClaudeAgentOptions builder

## Dependencies

- ✓ rusty_claw-9pf: Define error hierarchy (COMPLETE)

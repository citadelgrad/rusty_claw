# Current Task: rusty_claw-9pf

## Define error hierarchy

**Status:** IN_PROGRESS
**Priority:** P1
**Task ID:** rusty_claw-9pf

## Description

Implement `ClawError` enum using `thiserror` with variants for:
- CliNotFound
- Connection
- Process
- JsonDecode
- MessageParse
- ControlTimeout
- ControlError
- Io
- ToolExecution

## Blocks

- rusty_claw-6cn: Implement Transport trait and SubprocessCLITransport
- rusty_claw-pwc: Define shared types and message structs
- rusty_claw-k71: Implement CLI discovery and version check

## Dependencies

- ✓ rusty_claw-eia: Set up workspace and crate structure (COMPLETE)

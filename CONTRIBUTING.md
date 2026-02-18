# Contributing to Rusty Claw

Thank you for your interest in contributing! This document covers everything you need to get started.

## Getting Started

### Prerequisites

- **Rust** 1.70 or later
- **Claude CLI** v2.0.0 or later ([install guide](https://docs.anthropic.com/claude/docs/claude-cli))

### Setup

```bash
git clone https://github.com/citadelgrad/rusty_claw.git
cd rusty_claw
cargo build
```

### Project Structure

```
rusty_claw/
├── crates/
│   ├── rusty_claw/            # Core SDK crate
│   │   ├── src/
│   │   │   ├── client.rs      # ClaudeClient for interactive sessions
│   │   │   ├── control/       # Claude Control Protocol (CCP)
│   │   │   ├── hooks/         # Lifecycle event hooks
│   │   │   ├── mcp_server.rs  # Model Context Protocol integration
│   │   │   ├── messages.rs    # Message types and deserialization
│   │   │   ├── options.rs     # Configuration options and builder
│   │   │   ├── permissions/   # Permission handling
│   │   │   ├── query.rs       # One-shot query API
│   │   │   └── transport/     # Subprocess JSONL transport
│   │   ├── examples/          # Crate-level examples
│   │   ├── tests/             # Integration tests
│   │   └── test_support/      # Mock CLI for testing
│   └── rusty_claw_macros/     # #[claw_tool] proc macro crate
├── examples/                  # Standalone example crate
└── docs/                      # Specifications and guides
```

## Development Workflow

### Running Tests

```bash
# All tests
cargo test --workspace

# A specific crate
cargo test -p rusty_claw
cargo test -p rusty_claw_macros
```

### Linting

```bash
cargo clippy --workspace
```

### Formatting

```bash
# Check formatting
cargo fmt --check

# Apply formatting
cargo fmt
```

### Building Documentation

```bash
cargo doc --workspace --no-deps --open
```

### Running Examples

```bash
# Standalone examples (examples/ crate)
cargo run -p examples --example simple_query
cargo run -p examples --example interactive_client
cargo run -p examples --example custom_tool

# Crate-level examples
cargo run -p rusty_claw --example hooks_guardrails
cargo run -p rusty_claw --example subagent_usage
```

Note: All examples require the Claude CLI (>= 2.0.0) and a valid `ANTHROPIC_API_KEY`.

## Making Changes

1. Fork the repository and create a branch from `main`.
2. Make your changes.
3. Add or update tests for any new or changed behavior.
4. Run `cargo test --workspace`, `cargo clippy --workspace`, and `cargo fmt --check`.
5. Open a pull request against `main`.

### Commit Messages

Write concise commit messages that focus on **why** the change was made:

```
Fix runtime panic in messages() by using std::sync::Mutex

The messages_rx field used tokio::sync::Mutex with rt.block_on() in a
sync trait method, which panics inside an async context.
```

### What Makes a Good PR

- Focused on a single concern
- Includes tests for new functionality
- Passes all CI checks
- Updates documentation if public API changes

## Code Conventions

- Follow standard Rust idioms and the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/).
- Use `thiserror` for error types — all public errors go through `ClawError`.
- Async code uses `tokio` and `async-trait`.
- Proc macro code lives in `rusty_claw_macros`; the main crate re-exports it.
- Public types and functions should have doc comments with examples where useful.

## Reporting Issues

Open an issue on [GitHub](https://github.com/citadelgrad/rusty_claw/issues) with:

- A clear description of the problem or feature request
- Steps to reproduce (for bugs)
- Rust version and platform

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).

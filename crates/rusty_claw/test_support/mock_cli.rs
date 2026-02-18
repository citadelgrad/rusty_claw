//! Mock CLI binary for integration testing
//!
//! This binary replays canned NDJSON responses from fixture files, simulating the
//! behavior of the real `claude` CLI for integration testing purposes.
//!
//! # Usage
//!
//! ```bash
//! mock_cli --fixture=path/to/fixture.ndjson
//! ```
//!
//! The binary will:
//! 1. Load the specified NDJSON fixture file
//! 2. Parse and validate each line as JSON
//! 3. Write each line to stdout with realistic timing delays
//! 4. Flush stdout after each line (required for NDJSON streaming)
//! 5. Exit with code 0 on success
//!
//! # Signal Handling
//!
//! - SIGTERM/SIGINT → graceful shutdown
//! - Broken pipe → graceful exit (client disconnected)
//!
//! # Special Flags
//!
//! - `--version` → print version and exit
//! - `--help` → print help text and exit

use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process;
use std::thread;
use std::time::Duration;

const VERSION: &str = "2.0.0 (Mock Claude Code)";
const HELP_TEXT: &str = r#"mock_cli - Mock Claude CLI for integration testing

USAGE:
    mock_cli [OPTIONS]

OPTIONS:
    --fixture=<PATH>     Path to NDJSON fixture file (required)
    --delay=<MS>         Delay between messages in milliseconds (default: 20)
    --version            Print version and exit
    --help               Print this help text and exit

EXAMPLES:
    mock_cli --fixture=tests/fixtures/simple_query.ndjson
    mock_cli --fixture=simple_query.ndjson --delay=50
"#;

/// Parse command-line arguments manually (no external dependencies)
struct CliArgs {
    fixture_path: Option<PathBuf>,
    delay_ms: u64,
}

impl CliArgs {
    fn parse() -> Result<Self, String> {
        let args: Vec<String> = env::args().collect();

        let mut fixture_path = None;
        let mut delay_ms = 20; // default delay

        for arg in args.iter().skip(1) {
            if arg == "--version" {
                println!("{}", VERSION);
                process::exit(0);
            } else if arg == "--help" {
                println!("{}", HELP_TEXT);
                process::exit(0);
            } else if let Some(path) = arg.strip_prefix("--fixture=") {
                fixture_path = Some(PathBuf::from(path));
            } else if let Some(delay_str) = arg.strip_prefix("--delay=") {
                delay_ms = delay_str
                    .parse()
                    .map_err(|_| format!("Invalid delay value: {}", delay_str))?;
            } else if arg.starts_with("--output-format=") || arg.starts_with("--model=") {
                // Ignore output-format and model flags (used by real CLI)
                // These are passed through by tests but not needed by mock
            } else {
                return Err(format!("Unknown argument: {}", arg));
            }
        }

        Ok(CliArgs {
            fixture_path,
            delay_ms,
        })
    }
}

/// Load and validate NDJSON fixture file
fn load_fixture(path: &PathBuf) -> io::Result<Vec<String>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut lines = Vec::new();

    for (line_num, line_result) in reader.lines().enumerate() {
        let line = line_result?;

        // Skip empty lines
        if line.trim().is_empty() {
            continue;
        }

        // Validate JSON before adding to output
        if let Err(err) = serde_json::from_str::<serde_json::Value>(&line) {
            eprintln!("ERROR: Invalid JSON at line {}: {}", line_num + 1, err);
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid JSON at line {}", line_num + 1),
            ));
        }

        lines.push(line);
    }

    Ok(lines)
}

/// Replay NDJSON lines to stdout with realistic timing
fn replay_fixture(lines: Vec<String>, delay_ms: u64) -> io::Result<()> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    for line in lines {
        // Write line with newline
        writeln!(handle, "{}", line)?;

        // Flush immediately (critical for NDJSON streaming)
        handle.flush()?;

        // Simulate realistic delay between messages
        if delay_ms > 0 {
            thread::sleep(Duration::from_millis(delay_ms));
        }
    }

    Ok(())
}

fn main() {
    // Signal handling (SIGTERM/SIGINT) is handled at OS level
    // No need for explicit handlers - broken pipe is checked in replay_fixture()

    // Parse CLI args
    let args = match CliArgs::parse() {
        Ok(args) => args,
        Err(err) => {
            eprintln!("ERROR: {}", err);
            eprintln!("\nRun 'mock_cli --help' for usage information.");
            process::exit(1);
        }
    };

    // Require fixture path
    let fixture_path = match args.fixture_path {
        Some(path) => path,
        None => {
            eprintln!("ERROR: Missing required argument --fixture=<PATH>");
            eprintln!("\nRun 'mock_cli --help' for usage information.");
            process::exit(1);
        }
    };

    // Load fixture
    let lines = match load_fixture(&fixture_path) {
        Ok(lines) => lines,
        Err(err) => {
            eprintln!("ERROR: Failed to load fixture: {}", err);
            process::exit(1);
        }
    };

    // Replay fixture to stdout
    if let Err(err) = replay_fixture(lines, args.delay_ms) {
        // Broken pipe is OK (client disconnected)
        if err.kind() != io::ErrorKind::BrokenPipe {
            eprintln!("ERROR: Failed to replay fixture: {}", err);
            process::exit(1);
        }
    }

    // Exit successfully
    process::exit(0);
}

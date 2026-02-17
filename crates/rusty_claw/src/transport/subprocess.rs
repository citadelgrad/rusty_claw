//! Subprocess-based transport implementation
//!
//! This module provides [`SubprocessCLITransport`], which spawns the `claude` CLI
//! as a child process and communicates over stdin/stdout pipes.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, Mutex};

use tracing::{debug, error, trace, warn};

use crate::error::ClawError;
use crate::transport::Transport;

/// Type alias for the message receiver channel
type MessageReceiver = mpsc::UnboundedReceiver<Result<Value, ClawError>>;

/// Transport implementation that spawns Claude CLI as a subprocess
///
/// # Process Lifecycle
///
/// 1. **Construction** - Store CLI path and arguments
/// 2. **Connection** - Spawn subprocess with piped stdin/stdout/stderr
/// 3. **Communication** - Background tasks handle I/O:
///    - Reader task: Parse NDJSON from stdout → send to channel
///    - Monitor task: Detect unexpected process exits
/// 4. **Shutdown** - Graceful: close stdin → wait. Forced: SIGTERM → SIGKILL
///
/// # Thread Safety
///
/// All public methods are safe to call concurrently:
/// - `write()` uses `Arc<Mutex<>>` for stdin access
/// - `is_ready()` uses atomic operations
/// - Background tasks coordinate via channels and atomics
///
/// # Example
///
/// ```ignore
/// let mut transport = SubprocessCLITransport::new(
///     PathBuf::from("claude"),
///     vec![
///         "--output-format=stream-json".to_string(),
///         "--verbose".to_string(),
///     ]
/// );
///
/// transport.connect().await?;
/// assert!(transport.is_ready());
/// ```
pub struct SubprocessCLITransport {
    /// Process ID (set during connect, used for signal-based shutdown)
    child_pid: Option<u32>,

    /// Stdin handle wrapped for concurrent access
    stdin: Arc<Mutex<Option<ChildStdin>>>,

    /// Message receiver (moved out on first call to messages())
    messages_rx: Arc<std::sync::Mutex<Option<MessageReceiver>>>,

    /// Connection state (true if process is alive and connected)
    connected: Arc<AtomicBool>,

    /// Optional explicit CLI path (used for discovery on connect)
    cli_path_arg: Option<PathBuf>,

    /// Resolved CLI path (set during connect)
    cli_path: Arc<Mutex<Option<PathBuf>>>,

    /// Arguments to pass to CLI
    args: Vec<String>,

    /// Captured stderr for error diagnostics
    stderr_buffer: Arc<Mutex<String>>,
}

impl SubprocessCLITransport {
    /// Create a new subprocess transport
    ///
    /// # Arguments
    ///
    /// * `cli_path` - Optional path to the `claude` CLI executable.
    ///   If `None`, the CLI will be auto-discovered during [`connect()`](Transport::connect).
    /// * `args` - Command-line arguments (should include `--output-format=stream-json`)
    ///
    /// # Example
    ///
    /// ```
    /// use std::path::PathBuf;
    /// use rusty_claw::transport::SubprocessCLITransport;
    ///
    /// // Auto-discover CLI from PATH
    /// let transport = SubprocessCLITransport::new(
    ///     None,
    ///     vec!["--output-format=stream-json".to_string()]
    /// );
    ///
    /// // Or use explicit path
    /// let transport = SubprocessCLITransport::new(
    ///     Some(PathBuf::from("/opt/homebrew/bin/claude")),
    ///     vec!["--output-format=stream-json".to_string()]
    /// );
    /// ```
    pub fn new(cli_path: Option<PathBuf>, args: Vec<String>) -> Self {
        Self {
            child_pid: None,
            stdin: Arc::new(Mutex::new(None)),
            messages_rx: Arc::new(std::sync::Mutex::new(None)),
            connected: Arc::new(AtomicBool::new(false)),
            cli_path_arg: cli_path,
            cli_path: Arc::new(Mutex::new(None)),
            args,
            stderr_buffer: Arc::new(Mutex::new(String::new())),
        }
    }

    /// Spawn background task to read stdout and parse NDJSON messages
    fn spawn_reader_task(
        stdout: tokio::process::ChildStdout,
        tx: mpsc::UnboundedSender<Result<Value, ClawError>>,
        connected: Arc<AtomicBool>,
    ) {
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            debug!("Started stdout reader task");

            while let Ok(Some(line)) = lines.next_line().await {
                trace!("Received line: {}", line);

                // Skip empty lines
                if line.trim().is_empty() {
                    continue;
                }

                // Parse JSON
                match serde_json::from_str::<Value>(&line) {
                    Ok(value) => {
                        if tx.send(Ok(value)).is_err() {
                            debug!("Message receiver dropped, stopping reader task");
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse JSON line '{}': {}", line, e);
                        if tx.send(Err(ClawError::JsonDecode(e))).is_err() {
                            debug!("Message receiver dropped, stopping reader task");
                            break;
                        }
                    }
                }
            }

            debug!("Stdout reader task finished");
            connected.store(false, Ordering::SeqCst);
        });
    }

    /// Spawn background task to read stderr for diagnostics
    fn spawn_stderr_task(
        stderr: tokio::process::ChildStderr,
        buffer: Arc<Mutex<String>>,
    ) {
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();

            debug!("Started stderr reader task");

            while let Ok(Some(line)) = lines.next_line().await {
                warn!("CLI stderr: {}", line);
                let mut buf = buffer.lock().await;
                buf.push_str(&line);
                buf.push('\n');
            }

            debug!("Stderr reader task finished");
        });
    }

    /// Spawn background task to monitor process health
    fn spawn_monitor_task(
        mut child: Child,
        connected: Arc<AtomicBool>,
        stderr_buffer: Arc<Mutex<String>>,
    ) -> tokio::task::JoinHandle<Result<(), ClawError>> {
        tokio::spawn(async move {
            let status = child.wait().await.map_err(ClawError::Io)?;

            debug!("Process exited with status: {:?}", status);
            connected.store(false, Ordering::SeqCst);

            if !status.success() {
                let stderr = stderr_buffer.lock().await.clone();
                return Err(ClawError::Process {
                    code: status.code().unwrap_or(-1),
                    stderr,
                });
            }

            Ok(())
        })
    }

    /// Perform graceful shutdown: close stdin, wait briefly, then signal
    async fn graceful_shutdown(&mut self) -> Result<(), ClawError> {
        debug!("Starting graceful shutdown");

        // Close stdin first to signal the CLI to exit
        self.end_input().await?;

        // Give the process a moment to exit gracefully after stdin closes
        tokio::time::sleep(Duration::from_millis(500)).await;

        // If still connected, escalate to signal-based shutdown
        if self.connected.load(Ordering::SeqCst) {
            if let Some(pid) = self.child_pid {
                debug!("Process still running after stdin close, sending signals to pid {}", pid);
                self.force_shutdown_by_pid(pid).await?;
            }
        }

        Ok(())
    }

    /// Force shutdown using PID: SIGTERM → wait → SIGKILL
    async fn force_shutdown_by_pid(&self, pid: u32) -> Result<(), ClawError> {
        #[cfg(unix)]
        {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;

            let nix_pid = Pid::from_raw(pid as i32);

            debug!("Sending SIGTERM to pid {}", pid);
            let _ = kill(nix_pid, Signal::SIGTERM);

            // Wait up to 5s for SIGTERM to take effect
            for _ in 0..50 {
                tokio::time::sleep(Duration::from_millis(100)).await;
                if !self.connected.load(Ordering::SeqCst) {
                    debug!("Process exited after SIGTERM");
                    return Ok(());
                }
            }

            // SIGTERM failed, use SIGKILL
            warn!("SIGTERM timed out, sending SIGKILL to pid {}", pid);
            let _ = kill(nix_pid, Signal::SIGKILL);
        }

        #[cfg(not(unix))]
        {
            // On non-Unix, kill_on_drop(true) will handle cleanup when transport is dropped
            warn!("Signal-based shutdown not available on non-Unix; relying on kill_on_drop");
            let _ = pid;
        }

        Ok(())
    }
}

#[async_trait]
impl Transport for SubprocessCLITransport {
    async fn connect(&mut self) -> Result<(), ClawError> {
        if self.connected.load(Ordering::SeqCst) {
            return Err(ClawError::Connection(
                "already connected".to_string(),
            ));
        }

        // Discover and validate CLI
        let cli_path = {
            let mut guard = self.cli_path.lock().await;
            if guard.is_none() {
                use crate::transport::CliDiscovery;

                // Find CLI using discovery logic
                let discovered = CliDiscovery::find(self.cli_path_arg.as_deref()).await?;

                // Validate version >= 2.0.0
                let version = CliDiscovery::validate_version(&discovered).await?;
                debug!("Using CLI at {} (version {})", discovered.display(), version);

                *guard = Some(discovered.clone());
                discovered
            } else {
                guard.clone().unwrap()
            }
        };

        debug!("Spawning CLI: {} {:?}", cli_path.display(), self.args);

        // Spawn subprocess
        let mut cmd = Command::new(&cli_path);
        cmd.args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ClawError::CliNotFound
            } else {
                ClawError::Io(e)
            }
        })?;

        let child_pid = child.id();
        debug!("Process spawned with pid: {:?}", child_pid);
        self.child_pid = child_pid;

        // Take stdin/stdout/stderr handles
        let stdin = child.stdin.take().ok_or_else(|| {
            ClawError::Connection("failed to capture stdin".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            ClawError::Connection("failed to capture stdout".to_string())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            ClawError::Connection("failed to capture stderr".to_string())
        })?;

        // Set up message channel
        let (tx, rx) = mpsc::unbounded_channel();
        *self.messages_rx.lock().unwrap() = Some(rx);

        // Store stdin
        *self.stdin.lock().await = Some(stdin);

        // Spawn background tasks
        Self::spawn_reader_task(stdout, tx, self.connected.clone());
        Self::spawn_stderr_task(stderr, self.stderr_buffer.clone());
        let _monitor = Self::spawn_monitor_task(
            child,
            self.connected.clone(),
            self.stderr_buffer.clone(),
        );

        self.connected.store(true, Ordering::SeqCst);
        debug!("Connection established");

        Ok(())
    }

    async fn write(&self, message: &[u8]) -> Result<(), ClawError> {
        if !self.is_ready() {
            return Err(ClawError::Connection("not connected".to_string()));
        }

        let mut stdin_guard = self.stdin.lock().await;
        let stdin = stdin_guard.as_mut().ok_or_else(|| {
            ClawError::Connection("stdin already closed".to_string())
        })?;

        trace!("Writing {} bytes to stdin", message.len());

        stdin.write_all(message).await.map_err(ClawError::Io)?;

        stdin.flush().await.map_err(ClawError::Io)?;

        Ok(())
    }

    fn messages(&self) -> MessageReceiver {
        self.messages_rx.lock().unwrap().take()
            .expect("messages() can only be called once per connection")
    }

    async fn end_input(&self) -> Result<(), ClawError> {
        debug!("Closing stdin");

        let mut stdin_guard = self.stdin.lock().await;
        if let Some(mut stdin) = stdin_guard.take() {
            stdin.shutdown().await.map_err(ClawError::Io)?;
        }

        Ok(())
    }

    async fn close(&mut self) -> Result<(), ClawError> {
        if !self.connected.load(Ordering::SeqCst) {
            debug!("Already closed");
            return Ok(());
        }

        // Perform graceful shutdown first (needs connected=true to check process liveness)
        let result = self.graceful_shutdown().await;
        // Mark as disconnected after shutdown completes
        self.connected.store(false, Ordering::SeqCst);
        result
    }

    fn is_ready(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }
}

impl Drop for SubprocessCLITransport {
    fn drop(&mut self) {
        // Non-blocking cleanup: just mark as disconnected
        // The monitor task and kill_on_drop will handle process cleanup
        self.connected.store(false, Ordering::SeqCst);
        debug!("SubprocessCLITransport dropped");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_transport() {
        let transport = SubprocessCLITransport::new(
            Some(PathBuf::from("claude")),
            vec!["--output-format=stream-json".to_string()],
        );

        assert!(!transport.is_ready());
        assert_eq!(transport.cli_path_arg, Some(PathBuf::from("claude")));
        assert_eq!(transport.args.len(), 1);
    }

    #[test]
    fn test_not_ready_before_connect() {
        let transport = SubprocessCLITransport::new(
            None,
            vec![],
        );

        assert!(!transport.is_ready());
    }

    #[tokio::test]
    async fn test_write_when_not_connected() {
        let transport = SubprocessCLITransport::new(
            None,
            vec![],
        );

        let result = transport.write(b"test").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ClawError::Connection(_)));
    }

    #[tokio::test]
    async fn test_end_input_when_not_connected() {
        let transport = SubprocessCLITransport::new(
            None,
            vec![],
        );

        // Should not error (idempotent)
        let result = transport.end_input().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_close_when_not_connected() {
        let mut transport = SubprocessCLITransport::new(
            None,
            vec![],
        );

        // Should not error (idempotent)
        let result = transport.close().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_connect_with_invalid_cli() {
        // Create temp directory that doesn't contain claude
        let temp_dir = std::env::temp_dir().join("rusty_claw_test_invalid");
        std::fs::create_dir_all(&temp_dir).ok();
        let invalid_path = temp_dir.join("nonexistent_claude_binary");

        let mut transport = SubprocessCLITransport::new(
            Some(invalid_path),
            vec![],
        );

        let result = transport.connect().await;

        // If claude is installed elsewhere, it may be discovered via fallback
        // and the test might succeed. Only assert error if claude is not found.
        if result.is_err() {
            let err = result.unwrap_err();
            assert!(
                matches!(err, ClawError::CliNotFound | ClawError::InvalidCliVersion { .. }),
                "Expected CliNotFound or InvalidCliVersion, got: {:?}", err
            );
        }
        // Otherwise claude was found via discovery - test passes
    }

    #[tokio::test]
    async fn test_double_connect_fails() {
        // Test double connect with None to trigger auto-discovery
        let mut transport = SubprocessCLITransport::new(
            None,
            vec![],
        );

        // If claude is installed, test double connect
        if transport.connect().await.is_ok() {
            let result2 = transport.connect().await;
            assert!(result2.is_err());
            assert!(matches!(result2.unwrap_err(), ClawError::Connection(_)));
        }
        // Otherwise, test is skipped (no claude installed in test environment)
    }
}

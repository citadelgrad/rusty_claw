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
use tokio::time::timeout;
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
    /// Child process handle (None until connected)
    child: Option<Child>,

    /// Stdin handle wrapped for concurrent access
    stdin: Arc<Mutex<Option<ChildStdin>>>,

    /// Message receiver (moved out on first call to messages())
    messages_rx: Arc<Mutex<Option<MessageReceiver>>>,

    /// Connection state (true if process is alive and connected)
    connected: Arc<AtomicBool>,

    /// Path to claude CLI executable
    cli_path: PathBuf,

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
    /// * `cli_path` - Path to the `claude` CLI executable
    /// * `args` - Command-line arguments (should include `--output-format=stream-json`)
    ///
    /// # Example
    ///
    /// ```
    /// use std::path::PathBuf;
    /// use rusty_claw::transport::SubprocessCLITransport;
    ///
    /// let transport = SubprocessCLITransport::new(
    ///     PathBuf::from("claude"),
    ///     vec!["--output-format=stream-json".to_string()]
    /// );
    /// ```
    pub fn new(cli_path: PathBuf, args: Vec<String>) -> Self {
        Self {
            child: None,
            stdin: Arc::new(Mutex::new(None)),
            messages_rx: Arc::new(Mutex::new(None)),
            connected: Arc::new(AtomicBool::new(false)),
            cli_path,
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

    /// Perform graceful shutdown: close stdin, wait for exit
    async fn graceful_shutdown(&mut self) -> Result<(), ClawError> {
        debug!("Starting graceful shutdown");

        // Close stdin first
        self.end_input().await?;

        // Wait for process to exit (with timeout)
        if let Some(mut child) = self.child.take() {
            match timeout(Duration::from_secs(5), child.wait()).await {
                Ok(Ok(status)) => {
                    debug!("Process exited gracefully with status: {:?}", status);
                    if !status.success() {
                        let stderr = self.stderr_buffer.lock().await.clone();
                        return Err(ClawError::Process {
                            code: status.code().unwrap_or(-1),
                            stderr,
                        });
                    }
                }
                Ok(Err(e)) => {
                    return Err(ClawError::Io(e));
                }
                Err(_) => {
                    warn!("Graceful shutdown timed out, sending SIGTERM");
                    return self.force_shutdown(child).await;
                }
            }
        }

        Ok(())
    }

    /// Force shutdown: SIGTERM → wait → SIGKILL
    async fn force_shutdown(&mut self, mut child: Child) -> Result<(), ClawError> {
        // Send SIGTERM
        #[cfg(unix)]
        {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;

            if let Some(pid) = child.id() {
                debug!("Sending SIGTERM to pid {}", pid);
                let _ = kill(Pid::from_raw(pid as i32), Signal::SIGTERM);

                // Wait up to 5s for SIGTERM to work
                match timeout(Duration::from_secs(5), child.wait()).await {
                    Ok(Ok(status)) => {
                        debug!("Process exited after SIGTERM: {:?}", status);
                        return Ok(());
                    }
                    Ok(Err(e)) => {
                        warn!("Error waiting after SIGTERM: {}", e);
                    }
                    Err(_) => {
                        warn!("SIGTERM timed out, sending SIGKILL");
                    }
                }

                // SIGTERM failed, use SIGKILL
                debug!("Sending SIGKILL to pid {}", pid);
                let _ = kill(Pid::from_raw(pid as i32), Signal::SIGKILL);
            }
        }

        #[cfg(not(unix))]
        {
            debug!("Force killing process (non-Unix platform)");
            let _ = child.kill().await;
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

        debug!("Spawning CLI: {} {:?}", self.cli_path.display(), self.args);

        // Spawn subprocess
        let mut cmd = Command::new(&self.cli_path);
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

        debug!("Process spawned with pid: {:?}", child.id());

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
        *self.messages_rx.lock().await = Some(rx);

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
        // Block on the lock to take the receiver
        // This is safe because we're in a sync context (non-async trait method)
        let rt = tokio::runtime::Handle::try_current()
            .expect("messages() must be called from within a tokio runtime");

        rt.block_on(async {
            self.messages_rx.lock().await.take()
                .expect("messages() can only be called once per connection")
        })
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

        self.connected.store(false, Ordering::SeqCst);
        self.graceful_shutdown().await
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
            PathBuf::from("claude"),
            vec!["--output-format=stream-json".to_string()],
        );

        assert!(!transport.is_ready());
        assert_eq!(transport.cli_path, PathBuf::from("claude"));
        assert_eq!(transport.args.len(), 1);
    }

    #[test]
    fn test_not_ready_before_connect() {
        let transport = SubprocessCLITransport::new(
            PathBuf::from("claude"),
            vec![],
        );

        assert!(!transport.is_ready());
    }

    #[tokio::test]
    async fn test_write_when_not_connected() {
        let transport = SubprocessCLITransport::new(
            PathBuf::from("claude"),
            vec![],
        );

        let result = transport.write(b"test").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ClawError::Connection(_)));
    }

    #[tokio::test]
    async fn test_end_input_when_not_connected() {
        let transport = SubprocessCLITransport::new(
            PathBuf::from("claude"),
            vec![],
        );

        // Should not error (idempotent)
        let result = transport.end_input().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_close_when_not_connected() {
        let mut transport = SubprocessCLITransport::new(
            PathBuf::from("claude"),
            vec![],
        );

        // Should not error (idempotent)
        let result = transport.close().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_connect_with_invalid_cli() {
        let mut transport = SubprocessCLITransport::new(
            PathBuf::from("/nonexistent/claude"),
            vec![],
        );

        let result = transport.connect().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ClawError::CliNotFound));
    }

    #[tokio::test]
    async fn test_double_connect_fails() {
        // Use echo as a simple CLI that exists
        let mut transport = SubprocessCLITransport::new(
            PathBuf::from("echo"),
            vec!["test".to_string()],
        );

        let result1 = transport.connect().await;
        assert!(result1.is_ok());

        let result2 = transport.connect().await;
        assert!(result2.is_err());
        assert!(matches!(result2.unwrap_err(), ClawError::Connection(_)));
    }
}

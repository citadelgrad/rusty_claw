//! Simple query API for one-shot Claude interactions
//!
//! The `query()` function provides a convenient way to send a prompt to Claude
//! and receive a stream of response messages.
//!
//! # Example
//!
//! ```ignore
//! use rusty_claw::query;
//! use rusty_claw::options::{ClaudeAgentOptions, PermissionMode};
//! use tokio_stream::StreamExt;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let options = ClaudeAgentOptions::builder()
//!         .max_turns(5)
//!         .permission_mode(PermissionMode::AcceptEdits)
//!         .build();
//!
//!     let mut stream = query("What files are in this directory?", Some(options)).await?;
//!
//!     while let Some(result) = stream.next().await {
//!         match result {
//!             Ok(msg) => println!("{:?}", msg),
//!             Err(e) => eprintln!("Error: {}", e),
//!         }
//!     }
//!     Ok(())
//! }
//! ```

use std::pin::Pin;
use std::task::{Context, Poll};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_stream::{Stream, StreamExt};

use crate::error::ClawError;
use crate::messages::Message;
use crate::options::ClaudeAgentOptions;
use crate::transport::{SubprocessCLITransport, Transport};

/// A stream wrapper that owns the transport to ensure proper lifetime management
///
/// This struct ensures that the `SubprocessCLITransport` stays alive as long as
/// the message stream is being consumed. When the stream is dropped, the transport
/// is also dropped, which gracefully shuts down the CLI subprocess.
pub struct QueryStream<S> {
    /// The underlying message stream
    inner: S,
    /// Transport instance (must outlive the stream)
    #[allow(dead_code)]
    transport: SubprocessCLITransport,
}

impl<S> QueryStream<S>
where
    S: Stream<Item = Result<Message, ClawError>>,
{
    /// Create a new query stream wrapping a transport and its message stream
    fn new(transport: SubprocessCLITransport, inner: S) -> Self {
        Self { inner, transport }
    }
}

impl<S> Stream for QueryStream<S>
where
    S: Stream<Item = Result<Message, ClawError>> + Unpin,
{
    type Item = Result<Message, ClawError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}

/// Execute a one-shot query to Claude and return a stream of messages
///
/// This function:
/// 1. Creates a SubprocessCLITransport (discovers CLI automatically)
/// 2. Connects to the CLI process
/// 3. Sends the prompt to the CLI
/// 4. Returns a stream of parsed Message structs
///
/// # Arguments
///
/// * `prompt` - The prompt string to send to Claude
/// * `options` - Optional configuration using `ClaudeAgentOptions`
///
/// # Returns
///
/// A stream of `Result<Message, ClawError>` that yields messages until the CLI closes.
/// The stream owns the transport, ensuring the CLI process stays alive while consuming messages.
///
/// # Errors
///
/// - `ClawError::CliNotFound` if Claude CLI is not found
/// - `ClawError::InvalidCliVersion` if CLI version < 2.0.0
/// - `ClawError::Connection` if transport fails to connect
/// - `ClawError::JsonDecode` if message parsing fails
/// - `ClawError::MessageParse` if message structure is invalid
///
/// # Example
///
/// ```ignore
/// use rusty_claw::query;
/// use rusty_claw::options::{ClaudeAgentOptions, PermissionMode};
/// use tokio_stream::StreamExt;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let options = ClaudeAgentOptions::builder()
///         .max_turns(5)
///         .permission_mode(PermissionMode::AcceptEdits)
///         .build();
///
///     let mut stream = query("What files are in this directory?", Some(options)).await?;
///
///     while let Some(result) = stream.next().await {
///         match result {
///             Ok(msg) => println!("{:?}", msg),
///             Err(e) => eprintln!("Error: {}", e),
///         }
///     }
///     Ok(())
/// }
/// ```
pub async fn query(
    prompt: impl Into<String>,
    options: Option<ClaudeAgentOptions>,
) -> Result<impl Stream<Item = Result<Message, ClawError>>, ClawError> {
    let prompt = prompt.into();

    // Extract CLI args from options or use defaults
    let args = if let Some(opts) = options {
        opts.to_cli_args(&prompt)
    } else {
        vec![
            "--output-format=stream-json".to_string(),
            "--verbose".to_string(),
            "--setting-sources=".to_string(),
            "-p".to_string(),
            prompt,
        ]
    };

    // Create transport with auto-discovery (None = discover CLI from PATH/env/common locations)
    let mut transport = SubprocessCLITransport::new(None, args);

    // Connect to CLI (discovers, validates version, spawns process)
    transport.connect().await?;

    // Close stdin to signal no more input (one-shot query uses -p flag for prompt)
    transport.end_input().await?;

    // Get the message receiver from transport
    let rx = transport.messages();

    // Convert receiver to stream and parse Message structs
    let stream = UnboundedReceiverStream::new(rx)
        .map(|result| {
            result.and_then(|value| {
                let raw = value.to_string();
                serde_json::from_value::<Message>(value).map_err(|e| {
                    ClawError::MessageParse {
                        reason: e.to_string(),
                        raw,
                    }
                })
            })
        });

    // Wrap in QueryStream to ensure transport outlives the stream
    Ok(QueryStream::new(transport, stream))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_stream_is_send() {
        // Verify that QueryStream implements Send (required for tokio tasks)
        fn assert_send<T: Send>() {}
        // Use a concrete type for the stream generic parameter
        use tokio_stream::wrappers::UnboundedReceiverStream;
        type ConcreteStream = UnboundedReceiverStream<Result<Message, ClawError>>;
        assert_send::<QueryStream<ConcreteStream>>();
    }

    #[test]
    fn test_query_stream_is_unpin() {
        // Verify that QueryStream implements Unpin (required for easy pinning)
        fn assert_unpin<T: Unpin>() {}
        use tokio_stream::wrappers::UnboundedReceiverStream;
        type ConcreteStream = UnboundedReceiverStream<Result<Message, ClawError>>;
        assert_unpin::<QueryStream<ConcreteStream>>();
    }

    #[test]
    fn test_query_accepts_string() {
        // Compile-time test: verify query accepts String
        fn _assert_compiles() {
            async fn _test() -> Result<(), ClawError> {
                let _ = query("test".to_string(), None).await?;
                Ok(())
            }
        }
    }

    #[test]
    fn test_query_accepts_str() {
        // Compile-time test: verify query accepts &str
        fn _assert_compiles() {
            async fn _test() -> Result<(), ClawError> {
                let _ = query("test", None).await?;
                Ok(())
            }
        }
    }
}

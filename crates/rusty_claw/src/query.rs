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

use serde_json::Value;
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
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--verbose".to_string(),
            "--setting-sources".to_string(),
            String::new(),
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
    let stream = UnboundedReceiverStream::new(rx).map(|result| {
        result.and_then(|value| {
            let raw = value.to_string();
            serde_json::from_value::<Message>(value).map_err(|e| ClawError::MessageParse {
                reason: e.to_string(),
                raw,
            })
        })
    });

    // Wrap in QueryStream to ensure transport outlives the stream
    Ok(QueryStream::new(transport, stream))
}


/// Execute a query that accepts a stream of input messages (multi-message input)
///
/// This function enables advanced agentic patterns where the initial input to Claude
/// consists of multiple messages — for example, injecting system context, tool results,
/// or a pre-built conversation history before the user's prompt.
///
/// Each item in `messages` is serialized as a NDJSON line and written to CLI stdin before
/// stdin is closed. The CLI processes these as the input conversation.
///
/// # Arguments
///
/// * `messages` - An async stream of `serde_json::Value` items to send as input messages
/// * `options` - Optional configuration using [`ClaudeAgentOptions`]
///
/// # Returns
///
/// A stream of `Result<Message, ClawError>` that yields messages until the CLI closes.
///
/// # Errors
///
/// - [`ClawError::CliNotFound`] if Claude CLI is not found
/// - [`ClawError::InvalidCliVersion`] if CLI version < 2.0.0
/// - [`ClawError::Connection`] if transport fails to connect
/// - [`ClawError::JsonDecode`] if message parsing fails
/// - [`ClawError::Io`] if writing messages to stdin fails
///
/// # Example
///
/// ```ignore
/// use rusty_claw::query::query_with_messages;
/// use rusty_claw::options::ClaudeAgentOptions;
/// use serde_json::json;
/// use tokio_stream::{StreamExt, iter as stream_iter};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Build an initial conversation with a system message + user prompt
///     let messages = vec![
///         json!({
///             "type": "user",
///             "message": {
///                 "role": "user",
///                 "content": [{"type": "text", "text": "What is 2 + 2?"}]
///             }
///         })
///     ];
///
///     let mut stream = query_with_messages(
///         stream_iter(messages),
///         None,
///     ).await?;
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
pub async fn query_with_messages(
    messages: impl Stream<Item = Value> + Unpin,
    options: Option<ClaudeAgentOptions>,
) -> Result<impl Stream<Item = Result<Message, ClawError>>, ClawError> {
    // Build CLI args WITHOUT -p (prompt comes via stdin in stream-json mode)
    let args = build_stream_args(options.as_ref());

    // Create transport with auto-discovery
    let mut transport = SubprocessCLITransport::new(
        options.as_ref().and_then(|o| o.cli_path.clone()),
        args,
    );

    // Apply working directory if configured
    if let Some(cwd) = options.as_ref().and_then(|o| o.cwd.as_ref()) {
        transport.set_cwd(cwd.clone());
    }

    // Apply environment variables if configured
    if let Some(env) = options.as_ref().map(|o| &o.env).filter(|e| !e.is_empty()) {
        transport.set_env(env.clone());
    }

    // Apply stderr callback if configured
    if let Some(cb) = options.as_ref().and_then(|o| o.stderr_callback.as_ref()) {
        let cb_clone = cb.clone();
        transport.set_stderr_callback(move |line| cb_clone(line));
    }

    // Apply max buffer size if configured
    if let Some(size) = options.as_ref().and_then(|o| o.max_buffer_size) {
        transport.set_max_buffer_size(size);
    }

    // Connect to CLI
    transport.connect().await?;

    // Send all input messages as NDJSON lines
    let mut messages = messages;
    while let Some(msg) = messages.next().await {
        let mut line = serde_json::to_string(&msg).map_err(ClawError::JsonDecode)?;
        line.push('\n');
        transport.write(line.as_bytes()).await?;
    }

    // Close stdin to signal end of input
    transport.end_input().await?;

    // Get the message receiver from transport
    let rx = transport.messages();

    // Convert receiver to stream and parse Message structs
    let stream = UnboundedReceiverStream::new(rx).map(|result| {
        result.and_then(|value| {
            let raw = value.to_string();
            serde_json::from_value::<Message>(value).map_err(|e| ClawError::MessageParse {
                reason: e.to_string(),
                raw,
            })
        })
    });

    // Wrap in QueryStream to ensure transport outlives the stream
    Ok(QueryStream::new(transport, stream))
}

/// Build CLI args for stream-json input mode (no -p flag)
fn build_stream_args(options: Option<&ClaudeAgentOptions>) -> Vec<String> {
    let mut args = vec![
        "--output-format".to_string(),
        "stream-json".to_string(),
        "--verbose".to_string(),
        "--input-format".to_string(),
        "stream-json".to_string(),
    ];

    if let Some(opts) = options {
        // Max turns
        if let Some(max_turns) = opts.max_turns {
            args.push("--max-turns".to_string());
            args.push(max_turns.to_string());
        }

        // Model
        if let Some(model) = &opts.model {
            args.push("--model".to_string());
            args.push(model.clone());
        }

        // Permission mode
        if let Some(mode) = &opts.permission_mode {
            args.push("--permission-mode".to_string());
            args.push(mode.to_cli_arg().to_string());
        }

        // System prompt
        if let Some(sys_prompt) = &opts.system_prompt {
            match sys_prompt {
                crate::options::SystemPrompt::Custom(text) => {
                    args.push("--system-prompt".to_string());
                    args.push(text.clone());
                }
                crate::options::SystemPrompt::Preset { preset } => {
                    args.push("--system-prompt-preset".to_string());
                    args.push(preset.clone());
                }
            }
        }

        // Allowed tools
        if !opts.allowed_tools.is_empty() {
            args.push("--allowed-tools".to_string());
            args.push(opts.allowed_tools.join(","));
        }

        // Betas (one per flag)
        for beta in &opts.betas {
            args.push("--beta".to_string());
            args.push(beta.clone());
        }

        // Model
        if let Some(fallback) = &opts.fallback_model {
            args.push("--fallback-model".to_string());
            args.push(fallback.clone());
        }

        // User identifier
        if let Some(user) = &opts.user {
            args.push("--user".to_string());
            args.push(user.clone());
        }

        // Settings isolation
        match &opts.setting_sources {
            Some(sources) => {
                args.push("--setting-sources".to_string());
                args.push(sources.join(","));
            }
            None => {
                args.push("--setting-sources".to_string());
                args.push(String::new());
            }
        }

        // Extra args escape hatch
        for (key, value) in &opts.extra_args {
            let flag = if key.starts_with("--") {
                key.clone()
            } else {
                format!("--{}", key)
            };
            args.push(flag);
            if let Some(val) = value {
                args.push(val.clone());
            }
        }
    } else {
        // Default: empty setting-sources for reproducibility
        args.push("--setting-sources".to_string());
        args.push(String::new());
    }

    args
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

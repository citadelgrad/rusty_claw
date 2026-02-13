//! Procedural macros for rusty_claw
//!
//! This crate provides procedural macros for the rusty_claw agent SDK:
//! - `#[claw_tool]`: Derive macro for MCP tool definitions
//!
//! These macros are re-exported by the main `rusty_claw` crate and should
//! typically be used through that interface.

use proc_macro::TokenStream;

/// Derive macro for defining MCP tools
///
/// This macro will be implemented in a future task to generate MCP tool
/// schemas from Rust function signatures.
///
/// # Example (planned)
///
/// ```ignore
/// #[claw_tool]
/// async fn my_tool(param: String) -> Result<String, Error> {
///     Ok(format!("Processed: {}", param))
/// }
/// ```
#[proc_macro_attribute]
pub fn claw_tool(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // Placeholder implementation - will be completed in future tasks
    // For now, just pass through the input unchanged
    item
}

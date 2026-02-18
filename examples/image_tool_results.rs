//! Image tool results — return rich content from MCP tools.
//!
//! MCP tools can return more than just text. This example demonstrates:
//! - `ToolContent::text()` — plain text content
//! - `ToolContent::image()` — base64-encoded image content
//! - `ToolResult::new()` — multi-content results combining text and images
//! - `ToolResult::error()` — error results
//!
//! ## Run
//! ```sh
//! cargo run -p examples --example image_tool_results
//! ```

use rusty_claw::mcp_server::{ToolContent, ToolResult};

fn main() {
    println!("=== Image Tool Results ===\n");

    // 1. Simple text result
    let text_result = ToolResult::text("Hello from a tool!");
    println!("Text result:");
    println!("  content: {:?}", text_result.content);
    println!("  is_error: {:?}", text_result.is_error);
    println!();

    // 2. Error result
    let error_result = ToolResult::error("File not found: /missing.txt");
    println!("Error result:");
    println!("  content: {:?}", error_result.content);
    println!("  is_error: {:?}", error_result.is_error);
    println!();

    // 3. Image content (base64-encoded)
    // In a real tool, you'd read an actual image and base64-encode it.
    let fake_png_base64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
    let image_content = ToolContent::image(fake_png_base64, "image/png");
    println!("Image content:");
    println!("  {:?}", image_content);
    println!();

    // 4. Multi-content result (text + image)
    let multi_result = ToolResult::new(vec![
        ToolContent::text("Screenshot of the application:"),
        ToolContent::image(fake_png_base64, "image/png"),
    ]);
    println!("Multi-content result:");
    println!("  {} content blocks:", multi_result.content.len());
    for (i, content) in multi_result.content.iter().enumerate() {
        match content {
            ToolContent::Text { text } => {
                println!("  [{}] Text: {:?}", i, text);
            }
            ToolContent::Image { mime_type, .. } => {
                println!("  [{}] Image: {} (base64 data omitted)", i, mime_type);
            }
        }
    }
    println!();

    // 5. Serialization roundtrip
    let json = serde_json::to_string_pretty(&multi_result).unwrap();
    println!("Serialized multi-content result:");
    println!("{}", json);
    println!();

    // 6. Real-world usage pattern in a #[claw_tool]
    println!("=== Usage in a #[claw_tool] handler ===");
    println!();
    println!("  #[claw_tool(name = \"screenshot\", description = \"Take a screenshot\")]");
    println!("  async fn screenshot(url: String) -> ToolResult {{");
    println!("      let image_data = capture_screenshot(&url).await;");
    println!("      let base64 = base64::encode(&image_data);");
    println!("      ToolResult::new(vec![");
    println!("          ToolContent::text(format!(\"Screenshot of {{}}\", url)),");
    println!("          ToolContent::image(base64, \"image/png\"),");
    println!("      ])");
    println!("  }}");
    println!();

    println!("Done — demonstrated text, image, error, and multi-content tool results.");
}

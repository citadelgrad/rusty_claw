# Investigation: rusty_claw-zyo - Implement #[claw_tool] proc macro

**Task ID:** rusty_claw-zyo
**Title:** Implement #[claw_tool] proc macro
**Priority:** P2 (High)
**Date:** 2026-02-13
**Status:** Investigation Complete

## Executive Summary

This task requires implementing a procedural attribute macro `#[claw_tool]` that automatically generates MCP tool definitions from annotated Rust functions. The macro will:

1. Parse function signatures and extract parameter names/types
2. Auto-derive JSON Schema for input_schema from parameters
3. Generate a builder function that returns `SdkMcpTool`
4. Generate a handler struct that implements `ToolHandler`
5. Validate that parameters are JSON-serializable
6. Handle errors gracefully with clear compile-time error messages

## Current State

### Existing Infrastructure ✅

**rusty_claw_macros crate exists:**
- Location: `crates/rusty_claw_macros/`
- Dependencies: `syn`, `quote`, `proc-macro2` (already in workspace)
- Current implementation: Placeholder that passes through input unchanged
- File: `src/lib.rs` (30 lines, stub implementation)

**Target types exist in rusty_claw:**
- `SdkMcpTool` struct - Located in `mcp_server.rs:320-456`
- `ToolHandler` trait - Located in `mcp_server.rs:274-287`
- `ToolResult` struct - Located in `mcp_server.rs:185-245`
- `ToolContent` enum - Located in `mcp_server.rs:119-166`

**MCP Server bridge complete (rusty_claw-tlh):**
- All dependencies satisfied ✅
- Ready for macro integration

### Specification from SPEC.md

**Input format:**
```rust
#[claw_tool(name = "lookup_user", description = "Look up a user by ID")]
async fn lookup_user(user_id: String) -> ToolResult {
    ToolResult::text(format!("Found user: {user_id}"))
}
```

**Expected expansion:**
```rust
fn lookup_user() -> SdkMcpTool {
    SdkMcpTool::new(
        "lookup_user",
        "Look up a user by ID",
        json!({
            "type": "object",
            "properties": {
                "user_id": { "type": "string" }
            },
            "required": ["user_id"]
        }),
        Arc::new(LookupUserHandler),
    )
}

struct LookupUserHandler;

#[async_trait]
impl ToolHandler for LookupUserHandler {
    async fn call(&self, args: serde_json::Value) -> Result<ToolResult, ClawError> {
        let user_id: String = serde_json::from_value(args["user_id"].clone())?;
        // Original function body
        Ok(ToolResult::text(format!("Found user: {user_id}")))
    }
}
```

## Implementation Plan

### Phase 1: Parse Macro Attributes and Function Signature (45 min)

**Goal:** Extract tool metadata and function parameters

**Tasks:**
1. Parse `name` and `description` from macro attributes
2. Extract function name (fallback if `name` not provided)
3. Parse function parameters (name, type)
4. Validate function signature (async, return type)
5. Handle doc comments for description fallback

**Files to modify:**
- `crates/rusty_claw_macros/src/lib.rs`

**Key considerations:**
- Must support both `name = "..."` and inferring from function name
- Must support both `description = "..."` and inferring from doc comments
- Function must be async
- Return type must be `ToolResult` or `Result<ToolResult, _>`

**Code structure:**
```rust
struct ClawToolArgs {
    name: Option<String>,
    description: Option<String>,
}

fn parse_claw_tool_args(attr: TokenStream) -> syn::Result<ClawToolArgs> {
    // Parse name = "...", description = "..." pairs
}

fn extract_doc_comment(attrs: &[syn::Attribute]) -> Option<String> {
    // Extract /// comments
}
```

### Phase 2: Generate JSON Schema from Parameters (60 min)

**Goal:** Convert Rust types to JSON Schema

**Tasks:**
1. Map common Rust types to JSON types
2. Handle Option<T> (not required)
3. Handle Vec<T> (arrays)
4. Handle nested types (basic support)
5. Generate required fields list

**Type mapping:**
```rust
String, &str         → "string"
i32, i64, u32, u64   → "number"
f32, f64             → "number"
bool                 → "boolean"
Vec<T>               → { "type": "array", "items": <T schema> }
Option<T>            → <T schema> (not in required list)
```

**Code structure:**
```rust
fn type_to_json_schema(ty: &syn::Type) -> TokenStream2 {
    match ty {
        syn::Type::Path(path) => {
            let ident = path.path.segments.last().unwrap().ident.to_string();
            match ident.as_str() {
                "String" => quote! { json!({"type": "string"}) },
                "i32" | "i64" | "u32" | "u64" | "f32" | "f64" =>
                    quote! { json!({"type": "number"}) },
                "bool" => quote! { json!({"type": "boolean"}) },
                "Option" => /* handle optional */,
                "Vec" => /* handle arrays */,
                _ => quote! { json!({"type": "object"}) }, // fallback
            }
        }
        _ => quote! { json!({"type": "object"}) }, // fallback
    }
}

fn generate_input_schema(params: &[FnParam]) -> TokenStream2 {
    // Generate JSON Schema with properties and required fields
}
```

### Phase 3: Generate Handler Struct and ToolHandler Impl (60 min)

**Goal:** Generate handler that wraps original function body

**Tasks:**
1. Generate unique handler struct name (e.g., `LookupUserHandler`)
2. Implement `ToolHandler` trait
3. Extract arguments from JSON
4. Call original function body
5. Handle error conversion

**Code structure:**
```rust
fn generate_handler(
    fn_name: &syn::Ident,
    params: &[FnParam],
    fn_body: &syn::Block,
) -> TokenStream2 {
    let handler_name = format_ident!("{}Handler", /* CamelCase fn_name */);

    // Generate argument extraction
    let arg_extractions = params.iter().map(|param| {
        let name = &param.name;
        let ty = &param.ty;
        quote! {
            let #name: #ty = serde_json::from_value(
                args[stringify!(#name)].clone()
            )?;
        }
    });

    quote! {
        struct #handler_name;

        #[async_trait::async_trait]
        impl rusty_claw::mcp_server::ToolHandler for #handler_name {
            async fn call(
                &self,
                args: serde_json::Value
            ) -> Result<rusty_claw::mcp_server::ToolResult, rusty_claw::ClawError> {
                #(#arg_extractions)*
                #fn_body
            }
        }
    }
}
```

### Phase 4: Generate Builder Function (30 min)

**Goal:** Generate function that returns SdkMcpTool

**Tasks:**
1. Generate function with original name
2. Construct SdkMcpTool with metadata
3. Wrap handler in Arc

**Code structure:**
```rust
fn generate_tool_builder(
    fn_name: &syn::Ident,
    tool_name: &str,
    description: &str,
    input_schema: TokenStream2,
    handler_name: &syn::Ident,
) -> TokenStream2 {
    quote! {
        pub fn #fn_name() -> rusty_claw::mcp_server::SdkMcpTool {
            rusty_claw::mcp_server::SdkMcpTool::new(
                #tool_name,
                #description,
                #input_schema,
                std::sync::Arc::new(#handler_name),
            )
        }
    }
}
```

### Phase 5: Error Handling and Validation (45 min)

**Goal:** Provide clear compile-time errors

**Tasks:**
1. Validate function is async
2. Validate return type
3. Validate parameter types (JSON-serializable)
4. Provide helpful error messages

**Validation checks:**
```rust
fn validate_function(func: &syn::ItemFn) -> syn::Result<()> {
    // Check for async
    if func.sig.asyncness.is_none() {
        return Err(syn::Error::new_spanned(
            &func.sig.fn_token,
            "#[claw_tool] requires async function"
        ));
    }

    // Check return type
    match &func.sig.output {
        syn::ReturnType::Type(_, ty) => {
            // Validate it's ToolResult or Result<ToolResult, _>
        }
        _ => return Err(syn::Error::new_spanned(
            &func.sig,
            "#[claw_tool] requires return type ToolResult or Result<ToolResult, E>"
        )),
    }

    Ok(())
}
```

### Phase 6: Integration Tests (60 min)

**Goal:** Verify macro works end-to-end

**Tasks:**
1. Create test file `crates/rusty_claw_macros/tests/integration.rs`
2. Test basic function with String parameter
3. Test function with multiple parameters
4. Test function with optional parameters
5. Test function with Vec parameters
6. Test error cases (compile_fail tests)

**Test structure:**
```rust
#[test]
fn test_basic_tool() {
    #[claw_tool(name = "echo", description = "Echo a message")]
    async fn echo_tool(message: String) -> ToolResult {
        ToolResult::text(message)
    }

    let tool = echo_tool();
    assert_eq!(tool.name, "echo");
    assert_eq!(tool.description, "Echo a message");
    // Validate schema
}

#[test]
fn test_multiple_params() {
    #[claw_tool]
    async fn add(a: i32, b: i32) -> ToolResult {
        ToolResult::text(format!("{}", a + b))
    }

    let tool = add();
    // Validate schema has both a and b
}

#[tokio::test]
async fn test_tool_execution() {
    #[claw_tool]
    async fn greet(name: String) -> ToolResult {
        ToolResult::text(format!("Hello, {}!", name))
    }

    let tool = greet();
    let result = tool.execute(json!({"name": "World"})).await.unwrap();
    // Validate result
}
```

### Phase 7: Documentation and Examples (45 min)

**Goal:** Comprehensive documentation

**Tasks:**
1. Update `lib.rs` documentation with examples
2. Add inline documentation for helper functions
3. Create example in doc comment
4. Document supported types
5. Document error cases

**Documentation sections:**
```rust
//! # Examples
//!
//! ## Basic usage
//!
//! ```
//! use rusty_claw::prelude::*;
//!
//! #[claw_tool(name = "greet", description = "Greet a user")]
//! async fn greet_user(name: String) -> ToolResult {
//!     ToolResult::text(format!("Hello, {}!", name))
//! }
//! ```
//!
//! ## Multiple parameters
//!
//! ```
//! #[claw_tool]
//! async fn calculate(x: i32, y: i32, operation: String) -> ToolResult {
//!     // ...
//! }
//! ```
//!
//! ## Optional parameters
//!
//! ```
//! #[claw_tool]
//! async fn search(query: String, limit: Option<i32>) -> ToolResult {
//!     // limit is not required in the schema
//! }
//! ```
```

### Phase 8: Final Testing and Verification (30 min)

**Goal:** Ensure quality standards

**Tasks:**
1. Run `cargo test --package rusty_claw_macros`
2. Run `cargo clippy --package rusty_claw_macros`
3. Run integration tests with main crate
4. Verify documentation builds
5. Test compilation with various parameter types

**Commands:**
```bash
# Unit tests
cargo test --package rusty_claw_macros

# Clippy
cargo clippy --package rusty_claw_macros -- -D warnings

# Doc tests
cargo test --package rusty_claw_macros --doc

# Full workspace test
cargo test --workspace
```

## Files to Create/Modify

### Modified Files (1 file)

**1. `crates/rusty_claw_macros/src/lib.rs`** (~400-500 lines)
- Current: 30 lines (placeholder)
- After: ~500 lines (full implementation)
- Changes:
  - Replace placeholder `claw_tool` function
  - Add attribute parsing
  - Add JSON schema generation
  - Add handler generation
  - Add builder function generation
  - Add validation logic
  - Add comprehensive documentation

### New Files (1 file)

**2. `crates/rusty_claw_macros/tests/integration.rs`** (~200 lines)
- Integration tests for macro expansion
- Test various parameter types
- Test error cases
- Test end-to-end execution

## Dependencies

### Satisfied ✅

- `syn` (workspace) - AST parsing
- `quote` (workspace) - Code generation
- `proc-macro2` (workspace) - Token manipulation
- `rusty_claw-tlh` (SDK MCP Server bridge) - COMPLETE

### Optional (not required)

- `schemars` - Could use for automatic JSON Schema generation, but manual implementation is sufficient
- Additional validation crates - Not needed for MVP

## Type Mapping Strategy

### Supported Types (Phase 1)

| Rust Type | JSON Schema | Required Trait |
|-----------|-------------|----------------|
| String | `{"type": "string"}` | Deserialize |
| &str | `{"type": "string"}` | N/A (not supported in params) |
| i32, i64, u32, u64 | `{"type": "number"}` | Deserialize |
| f32, f64 | `{"type": "number"}` | Deserialize |
| bool | `{"type": "boolean"}` | Deserialize |
| Option<T> | Same as T | Deserialize, not required |
| Vec<T> | `{"type": "array", "items": T}` | Deserialize |

### Future Extensions (Not in Scope)

- HashMap/BTreeMap → `{"type": "object"}`
- Custom structs with `#[derive(serde::Deserialize)]` → complex schema
- Tuples → array with fixed length
- Enums → oneOf schema

## Risk Assessment

### High Confidence ✅

1. **Macro infrastructure exists** - No need to set up crate
2. **Target types well-defined** - SdkMcpTool, ToolHandler, ToolResult are stable
3. **Dependencies available** - syn, quote, proc-macro2 in workspace
4. **Clear specification** - SPEC.md provides detailed example

### Medium Risk ⚠️

1. **Type mapping complexity**
   - **Mitigation:** Start with basic types, add complex types incrementally
   - **Fallback:** Default to `{"type": "object"}` for unknown types

2. **Error message quality**
   - **Mitigation:** Use `syn::Error::new_spanned` for precise error locations
   - **Testing:** Include compile_fail tests

3. **Async function handling**
   - **Mitigation:** Check for `asyncness` in signature validation
   - **Known pattern:** `async_trait` already used in codebase

### Low Risk ✅

1. **Integration with existing code** - Types already defined
2. **Testing infrastructure** - Can use standard Rust test framework
3. **Documentation** - Well-understood patterns from other macros

## Acceptance Criteria Mapping

| # | Criterion | Implementation Phase | Status |
|---|-----------|---------------------|--------|
| 1 | Functional #[claw_tool] macro | Phases 1-5 | ⏭️ |
| 2 | Auto-derive input_schema | Phase 2 | ⏭️ |
| 3 | Generate SdkMcpTool struct | Phase 4 | ⏭️ |
| 4 | Generate ToolHandler impl | Phase 3 | ⏭️ |
| 5 | Validate JSON-serializable params | Phase 5 | ⏭️ |
| 6 | Handle error cases gracefully | Phase 5 | ⏭️ |
| 7 | Integration tests with SDK | Phase 6 | ⏭️ |
| 8 | Zero clippy warnings | Phase 8 | ⏭️ |
| 9 | Comprehensive docs/examples | Phase 7 | ⏭️ |

## Timeline Estimate

| Phase | Duration | Cumulative |
|-------|----------|------------|
| 1. Parse attributes & signature | 45 min | 0:45 |
| 2. Generate JSON Schema | 60 min | 1:45 |
| 3. Generate handler impl | 60 min | 2:45 |
| 4. Generate builder function | 30 min | 3:15 |
| 5. Error handling | 45 min | 4:00 |
| 6. Integration tests | 60 min | 5:00 |
| 7. Documentation | 45 min | 5:45 |
| 8. Final verification | 30 min | 6:15 |

**Total Estimated Time:** ~6.25 hours

## Success Metrics

**Code Quality:**
- ✅ Zero clippy warnings
- ✅ All tests pass
- ✅ Doc tests pass
- ✅ Clear error messages

**Functionality:**
- ✅ Macro expands correctly for basic types
- ✅ Macro expands correctly for Option<T>
- ✅ Macro expands correctly for Vec<T>
- ✅ Generated tools are executable
- ✅ JSON Schema matches function signature

**Documentation:**
- ✅ Comprehensive doc comments
- ✅ Multiple examples in lib.rs
- ✅ Clear explanation of supported types
- ✅ Error case documentation

## Next Steps

1. ✅ Investigation complete
2. ⏭️ Phase 1: Parse macro attributes and function signature
3. ⏭️ Phase 2: Generate JSON Schema
4. ⏭️ Phase 3: Generate handler implementation
5. ⏭️ Phase 4: Generate builder function
6. ⏭️ Phase 5: Add error handling
7. ⏭️ Phase 6: Write integration tests
8. ⏭️ Phase 7: Add documentation
9. ⏭️ Phase 8: Final verification and commit

## References

- **SPEC.md:623-658** - Macro specification and expected expansion
- **mcp_server.rs:274-287** - ToolHandler trait definition
- **mcp_server.rs:320-456** - SdkMcpTool struct definition
- **mcp_server.rs:185-245** - ToolResult struct definition
- **Cargo.toml:38-41** - Proc macro dependencies (syn, quote, proc-macro2)

---

**Investigation Status:** ✅ COMPLETE
**Ready to Proceed:** YES
**Blockers:** NONE

The investigation is complete with clear understanding of:
- ✅ Existing infrastructure and types
- ✅ Expected macro behavior from SPEC.md
- ✅ Implementation approach (8 phases)
- ✅ Type mapping strategy
- ✅ Risk mitigation
- ✅ Success criteria

**Next step:** Phase 1 - Parse macro attributes and function signature! 🚀

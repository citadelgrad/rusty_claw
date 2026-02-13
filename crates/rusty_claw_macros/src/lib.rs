//! Procedural macros for rusty_claw
//!
//! This crate provides procedural macros for the rusty_claw agent SDK:
//! - `#[claw_tool]`: Attribute macro for MCP tool definitions
//!
//! These macros are re-exported by the main `rusty_claw` crate and should
//! typically be used through that interface.
//!
//! # Examples
//!
//! ## Basic usage
//!
//! ```ignore
//! use rusty_claw::prelude::*;
//! use rusty_claw::mcp_server::ToolResult;
//!
//! #[claw_tool(name = "greet", description = "Greet a user")]
//! async fn greet_user(name: String) -> ToolResult {
//!     ToolResult::text(format!("Hello, {}!", name))
//! }
//! ```
//!
//! ## Multiple parameters
//!
//! ```ignore
//! use rusty_claw::prelude::*;
//! use rusty_claw::mcp_server::ToolResult;
//!
//! #[claw_tool(name = "calculate", description = "Perform a calculation")]
//! async fn calculate(x: i32, y: i32, operation: String) -> ToolResult {
//!     let result = match operation.as_str() {
//!         "add" => x + y,
//!         "sub" => x - y,
//!         _ => 0,
//!     };
//!     ToolResult::text(format!("{}", result))
//! }
//! ```
//!
//! ## Optional parameters
//!
//! ```ignore
//! use rusty_claw::prelude::*;
//! use rusty_claw::mcp_server::ToolResult;
//!
//! #[claw_tool(name = "search", description = "Search with optional limit")]
//! async fn search(query: String, limit: Option<i32>) -> ToolResult {
//!     let max = limit.unwrap_or(10);
//!     ToolResult::text(format!("Searching for '{}' (limit: {})", query, max))
//! }
//! ```

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{
    parse::Parser, parse_macro_input, punctuated::Punctuated, Attribute, Block, Expr, ExprLit,
    FnArg, Ident, ItemFn, Lit, Meta, Pat, PatType, ReturnType, Token, Type,
};

/// Arguments parsed from the `#[claw_tool(...)]` attribute
struct ClawToolArgs {
    name: Option<String>,
    description: Option<String>,
}

impl ClawToolArgs {
    /// Parse arguments from the attribute token stream
    fn parse(attr: TokenStream2) -> syn::Result<Self> {
        let mut name = None;
        let mut description = None;

        if !attr.is_empty() {
            let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
            let metas = parser.parse2(attr)?;

            for meta in metas {
                match meta {
                    Meta::NameValue(nv) => {
                        let ident = nv
                            .path
                            .get_ident()
                            .ok_or_else(|| {
                                syn::Error::new_spanned(&nv.path, "Expected simple identifier")
                            })?
                            .to_string();

                        match ident.as_str() {
                            "name" => {
                                if let Expr::Lit(ExprLit {
                                    lit: Lit::Str(lit), ..
                                }) = &nv.value
                                {
                                    name = Some(lit.value());
                                } else {
                                    return Err(syn::Error::new_spanned(
                                        &nv.value,
                                        "Expected string literal for name",
                                    ));
                                }
                            }
                            "description" => {
                                if let Expr::Lit(ExprLit {
                                    lit: Lit::Str(lit), ..
                                }) = &nv.value
                                {
                                    description = Some(lit.value());
                                } else {
                                    return Err(syn::Error::new_spanned(
                                        &nv.value,
                                        "Expected string literal for description",
                                    ));
                                }
                            }
                            _ => {
                                return Err(syn::Error::new_spanned(
                                    &nv.path,
                                    format!("Unknown attribute '{}'", ident),
                                ));
                            }
                        }
                    }
                    _ => {
                        return Err(syn::Error::new_spanned(
                            &meta,
                            "Expected name = \"...\" format",
                        ));
                    }
                }
            }
        }

        Ok(Self { name, description })
    }
}

/// Parsed function parameter
struct FnParam {
    name: Ident,
    ty: Type,
    is_optional: bool,
}

impl FnParam {
    /// Parse function parameters from the function signature
    fn from_fn_args(inputs: &Punctuated<FnArg, Token![,]>) -> syn::Result<Vec<Self>> {
        let mut params = Vec::new();

        for input in inputs {
            match input {
                FnArg::Typed(PatType { pat, ty, .. }) => {
                    let name = match &**pat {
                        Pat::Ident(pat_ident) => pat_ident.ident.clone(),
                        _ => {
                            return Err(syn::Error::new_spanned(
                                pat,
                                "Only simple parameter names are supported",
                            ));
                        }
                    };

                    let is_optional = is_option_type(ty);

                    params.push(FnParam {
                        name,
                        ty: (**ty).clone(),
                        is_optional,
                    });
                }
                FnArg::Receiver(_) => {
                    return Err(syn::Error::new_spanned(
                        input,
                        "Self parameter not supported in #[claw_tool]",
                    ));
                }
            }
        }

        Ok(params)
    }
}

/// Check if a type is Option<T>
fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            return segment.ident == "Option";
        }
    }
    false
}

/// Extract inner type from Option<T>
fn extract_option_inner(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Option" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        return Some(inner_ty);
                    }
                }
            }
        }
    }
    None
}

/// Extract inner type from Vec<T>
fn extract_vec_inner(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Vec" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        return Some(inner_ty);
                    }
                }
            }
        }
    }
    None
}

/// Convert a Rust type to a JSON Schema token stream
fn type_to_json_schema(ty: &Type) -> TokenStream2 {
    // Handle Option<T> - unwrap and generate schema for T
    if let Some(inner_ty) = extract_option_inner(ty) {
        return type_to_json_schema(inner_ty);
    }

    // Handle Vec<T> - generate array schema
    if let Some(inner_ty) = extract_vec_inner(ty) {
        let inner_schema = type_to_json_schema(inner_ty);
        return quote! {
            serde_json::json!({
                "type": "array",
                "items": #inner_schema
            })
        };
    }

    // Handle basic types
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            let ident = segment.ident.to_string();
            return match ident.as_str() {
                "String" | "str" => quote! { serde_json::json!({"type": "string"}) },
                "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64"
                | "u128" | "usize" | "f32" | "f64" => {
                    quote! { serde_json::json!({"type": "number"}) }
                }
                "bool" => quote! { serde_json::json!({"type": "boolean"}) },
                _ => quote! { serde_json::json!({"type": "object"}) }, // Fallback for custom types
            };
        }
    }

    // Fallback for unknown types
    quote! { serde_json::json!({"type": "object"}) }
}

/// Generate the JSON Schema for function input parameters
fn generate_input_schema(params: &[FnParam]) -> TokenStream2 {
    let properties = params.iter().map(|param| {
        let name = param.name.to_string();
        let schema = type_to_json_schema(&param.ty);
        quote! {
            #name: #schema
        }
    });

    let required_fields = params
        .iter()
        .filter(|p| !p.is_optional)
        .map(|p| p.name.to_string());

    quote! {
        serde_json::json!({
            "type": "object",
            "properties": {
                #(#properties),*
            },
            "required": [#(#required_fields),*]
        })
    }
}

/// Generate the handler struct and ToolHandler implementation
fn generate_handler(
    fn_name: &Ident,
    params: &[FnParam],
    fn_body: &Block,
    returns_result: bool,
) -> syn::Result<TokenStream2> {
    // Create handler name: FunctionName -> FunctionNameHandler
    let fn_name_str = fn_name.to_string();
    let handler_name_str = fn_name_str
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<String>()
        + "Handler";

    let handler_name = Ident::new(&handler_name_str, Span::call_site());

    // Generate argument extraction code for each parameter
    let arg_extractions = params.iter().map(|param| {
        let name = &param.name;
        let name_str = name.to_string();
        let ty = &param.ty;

        if param.is_optional {
            // For Option<T>, use get() which returns Option
            quote! {
                let #name: #ty = args.get(#name_str)
                    .and_then(|v| serde_json::from_value(v.clone()).ok());
            }
        } else {
            // For required parameters, return error if missing or invalid
            quote! {
                let #name: #ty = serde_json::from_value(
                    args.get(#name_str)
                        .ok_or_else(|| rusty_claw::prelude::ClawError::ToolExecution(
                            format!("Missing required parameter '{}'", #name_str)
                        ))?
                        .clone()
                ).map_err(|e| rusty_claw::prelude::ClawError::ToolExecution(
                    format!("Invalid parameter '{}': {}", #name_str, e)
                ))?;
            }
        }
    });

    // If the function returns ToolResult (not Result<ToolResult, _>),
    // we need to wrap the body in Ok()
    let body = if returns_result {
        quote! { #fn_body }
    } else {
        quote! { Ok(#fn_body) }
    };

    Ok(quote! {
        struct #handler_name;

        #[async_trait::async_trait]
        impl rusty_claw::mcp_server::ToolHandler for #handler_name {
            async fn call(
                &self,
                args: serde_json::Value
            ) -> Result<rusty_claw::mcp_server::ToolResult, rusty_claw::prelude::ClawError> {
                #(#arg_extractions)*
                #body
            }
        }
    })
}

/// Generate the builder function that returns SdkMcpTool
fn generate_tool_builder(
    fn_name: &Ident,
    tool_name: &str,
    description: &str,
    input_schema: TokenStream2,
    handler_name: &Ident,
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

/// Extract documentation from doc comments
fn extract_doc_comment(attrs: &[Attribute]) -> Option<String> {
    let mut doc = String::new();

    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let Meta::NameValue(meta) = &attr.meta {
                if let Expr::Lit(ExprLit {
                    lit: Lit::Str(lit), ..
                }) = &meta.value
                {
                    let line = lit.value();
                    if !doc.is_empty() {
                        doc.push(' ');
                    }
                    doc.push_str(line.trim());
                }
            }
        }
    }

    if doc.is_empty() {
        None
    } else {
        Some(doc)
    }
}

/// Validate the function signature
fn validate_function(func: &ItemFn) -> syn::Result<()> {
    // Check for async
    if func.sig.asyncness.is_none() {
        return Err(syn::Error::new_spanned(
            func.sig.fn_token,
            "#[claw_tool] requires an async function",
        ));
    }

    // Check return type
    match &func.sig.output {
        ReturnType::Type(_, ty) => {
            // Check if it's ToolResult or Result<ToolResult, _>
            let is_valid = matches_type_name(ty, "ToolResult")
                || (is_result_type(ty)
                    && result_ok_type(ty).is_some_and(|ok_ty| matches_type_name(ok_ty, "ToolResult")));

            if !is_valid {
                return Err(syn::Error::new_spanned(
                    ty,
                    "#[claw_tool] requires return type ToolResult or Result<ToolResult, E>",
                ));
            }
        }
        ReturnType::Default => {
            return Err(syn::Error::new_spanned(
                &func.sig,
                "#[claw_tool] requires return type ToolResult or Result<ToolResult, E>",
            ));
        }
    }

    Ok(())
}

/// Check if a type matches a specific type name
fn matches_type_name(ty: &Type, name: &str) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            return segment.ident == name;
        }
    }
    false
}

/// Check if a type is Result<T, E>
fn is_result_type(ty: &Type) -> bool {
    matches_type_name(ty, "Result")
}

/// Extract the Ok type from Result<T, E>
fn result_ok_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Result" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(ok_ty)) = args.args.first() {
                        return Some(ok_ty);
                    }
                }
            }
        }
    }
    None
}

/// Attribute macro for defining MCP tools
///
/// This macro transforms an async function into an MCP tool definition with
/// auto-generated JSON Schema for input parameters.
///
/// # Arguments
///
/// * `name` - Optional tool name (defaults to function name)
/// * `description` - Optional tool description (defaults to doc comment)
///
/// # Requirements
///
/// * Function must be async
/// * Return type must be `ToolResult` or `Result<ToolResult, E>`
/// * Parameters must be JSON-serializable types
///
/// # Supported Parameter Types
///
/// * `String` - JSON string
/// * `i32`, `i64`, `u32`, `u64`, `f32`, `f64` - JSON number
/// * `bool` - JSON boolean
/// * `Option<T>` - Optional parameter (not required in schema)
/// * `Vec<T>` - JSON array
///
/// # Example
///
/// ```ignore
/// use rusty_claw::prelude::*;
/// use rusty_claw::mcp_server::ToolResult;
///
/// #[claw_tool(name = "echo", description = "Echo a message")]
/// async fn echo_tool(message: String) -> ToolResult {
///     ToolResult::text(message)
/// }
///
/// // Use the generated tool
/// let tool = echo_tool();
/// assert_eq!(tool.name, "echo");
/// ```
#[proc_macro_attribute]
pub fn claw_tool(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);

    // Parse the function and generate the tool definition
    let result = expand_claw_tool(attr.into(), input_fn);

    match result {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Core expansion logic for the claw_tool macro
fn expand_claw_tool(attr: TokenStream2, input_fn: ItemFn) -> syn::Result<TokenStream2> {
    // Validate function signature
    validate_function(&input_fn)?;

    // Parse macro arguments
    let args = ClawToolArgs::parse(attr)?;

    // Extract function metadata
    let fn_name = &input_fn.sig.ident;
    let fn_body = &input_fn.block;

    // Determine tool name (from attribute or function name)
    let tool_name = args
        .name
        .unwrap_or_else(|| fn_name.to_string().replace('_', "-"));

    // Determine description (from attribute or doc comment)
    let description = args
        .description
        .or_else(|| extract_doc_comment(&input_fn.attrs))
        .unwrap_or_else(|| format!("Tool: {}", tool_name));

    // Parse function parameters
    let params = FnParam::from_fn_args(&input_fn.sig.inputs)?;

    // Check if function returns Result<ToolResult, _> or just ToolResult
    let returns_result = if let ReturnType::Type(_, ty) = &input_fn.sig.output {
        is_result_type(ty)
    } else {
        false
    };

    // Generate JSON Schema for input
    let input_schema = generate_input_schema(&params);

    // Generate handler struct name
    let fn_name_str = fn_name.to_string();
    let handler_name_str = fn_name_str
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<String>()
        + "Handler";
    let handler_name = Ident::new(&handler_name_str, Span::call_site());

    // Generate handler implementation
    let handler_impl = generate_handler(fn_name, &params, fn_body, returns_result)?;

    // Generate builder function
    let builder_fn = generate_tool_builder(fn_name, &tool_name, &description, input_schema, &handler_name);

    // Combine all generated code
    Ok(quote! {
        #handler_impl
        #builder_fn
    })
}

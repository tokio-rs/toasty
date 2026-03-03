use syn::parse::discouraged::Speculative;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token;
use syn::{braced, bracketed};

/// Top-level macro input: `create!(Target, { fields })` or `create!(Target, [{ fields }, ...])`
pub(crate) struct CreateInput {
    pub target: Target,
    pub body: Body,
}

/// What is being created
pub(crate) enum Target {
    /// Type path like `User` → generates `User::create()`
    Type(syn::Path),
    /// Expression like `user.todos()` → generates `expr.create()`
    Scope(syn::Expr),
}

/// The fields/items body
pub(crate) enum Body {
    /// Single `{ k: v, ... }`
    Single(Vec<FieldEntry>),
    /// Batch `[{ k: v }, { k: v }, ...]`
    Batch(Vec<Vec<FieldEntry>>),
}

/// A single `name: value` pair
pub(crate) struct FieldEntry {
    pub name: syn::Ident,
    pub value: FieldValue,
}

/// Value in a field entry
pub(crate) enum FieldValue {
    /// Plain expression (literals, variables, etc.)
    Expr(syn::Expr),
    /// Nested struct-literal create: `Type { k: v }` → `Type::create().k(v)`
    Create {
        path: syn::Path,
        fields: Vec<FieldEntry>,
    },
    /// Array of values → repeated method calls
    List(Vec<FieldValue>),
}

impl Parse for CreateInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let target = parse_target(input)?;
        input.parse::<syn::Token![,]>()?;
        let body = input.parse::<Body>()?;
        Ok(CreateInput { target, body })
    }
}

/// Parse the target: try as a type path first, fall back to expression.
///
/// A target is a type path if it parses as a `syn::Path` followed by `,`.
/// Otherwise, it's an expression (e.g., `user.todos()`).
fn parse_target(input: ParseStream) -> syn::Result<Target> {
    // Fork the stream to try parsing as a Path
    let fork = input.fork();
    if let Ok(path) = fork.parse::<syn::Path>() {
        // Check that the path is followed by `,` (not more tokens like `.method()`)
        if fork.peek(syn::Token![,]) {
            // Advance the real stream past the path
            input.advance_to(&fork);
            return Ok(Target::Type(path));
        }
    }

    // Fall back to parsing as an expression
    let expr = input.parse::<syn::Expr>()?;
    Ok(Target::Scope(expr))
}

impl Parse for Body {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(token::Bracket) {
            // Batch: [{ ... }, { ... }]
            let content;
            bracketed!(content in input);
            let items = Punctuated::<BracedFields, syn::Token![,]>::parse_terminated(&content)?;
            Ok(Body::Batch(items.into_iter().map(|bf| bf.fields).collect()))
        } else {
            // Single: { ... }
            let fields = parse_braced_fields(input)?;
            Ok(Body::Single(fields))
        }
    }
}

/// Helper for parsing `{ field: value, ... }` — used in batch mode punctuated parsing
struct BracedFields {
    fields: Vec<FieldEntry>,
}

impl Parse for BracedFields {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let fields = parse_braced_fields(input)?;
        Ok(BracedFields { fields })
    }
}

/// Parse `{ field: value, field: value, ... }`
fn parse_braced_fields(input: ParseStream) -> syn::Result<Vec<FieldEntry>> {
    let content;
    braced!(content in input);
    let entries = Punctuated::<FieldEntry, syn::Token![,]>::parse_terminated(&content)?;
    Ok(entries.into_iter().collect())
}

impl Parse for FieldEntry {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        input.parse::<syn::Token![:]>()?;
        let value = parse_field_value(input)?;
        Ok(FieldEntry { name, value })
    }
}

/// Parse a field value, detecting nested creates and lists.
fn parse_field_value(input: ParseStream) -> syn::Result<FieldValue> {
    // Check for array literal: [...]
    if input.peek(token::Bracket) {
        let content;
        bracketed!(content in input);
        let values = Punctuated::<FieldValueItem, syn::Token![,]>::parse_terminated(&content)?;
        return Ok(FieldValue::List(values.into_iter().map(|v| v.0).collect()));
    }

    // Parse as a general expression
    let expr = input.parse::<syn::Expr>()?;

    // Post-process: detect struct literals → nested create
    Ok(classify_expr(expr))
}

/// Wrapper for parsing individual items in a field value list
struct FieldValueItem(FieldValue);

impl Parse for FieldValueItem {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Inside a list, check for nested braced fields: { k: v, ... }
        // This handles the case `[{ title: "a" }, { title: "b" }]` where
        // there's no type prefix (anonymous nested creates).
        if input.peek(token::Brace) {
            let fields = parse_braced_fields(input)?;
            // Anonymous nested create — no type path. Store as a Create with
            // an empty path. The expand phase will need to handle this
            // (likely an error or infer from context).
            return Ok(FieldValueItem(FieldValue::Create {
                path: syn::parse_quote! { __anonymous },
                fields,
            }));
        }

        let expr = input.parse::<syn::Expr>()?;
        Ok(FieldValueItem(classify_expr(expr)))
    }
}

/// Classify a parsed expression: detect struct literals and convert to FieldValue::Create.
fn classify_expr(expr: syn::Expr) -> FieldValue {
    match expr {
        syn::Expr::Struct(expr_struct) => {
            let path = expr_struct.path;
            let fields = expr_struct
                .fields
                .into_iter()
                .map(|fv| {
                    let syn::Member::Named(name) = fv.member else {
                        // Tuple-style fields not supported in create
                        return FieldEntry {
                            name: syn::Ident::new("__unknown", proc_macro2::Span::call_site()),
                            value: FieldValue::Expr(fv.expr),
                        };
                    };
                    FieldEntry {
                        name,
                        value: classify_expr(fv.expr),
                    }
                })
                .collect();
            FieldValue::Create { path, fields }
        }
        syn::Expr::Array(expr_array) => {
            let values = expr_array.elems.into_iter().map(classify_expr).collect();
            FieldValue::List(values)
        }
        other => FieldValue::Expr(other),
    }
}

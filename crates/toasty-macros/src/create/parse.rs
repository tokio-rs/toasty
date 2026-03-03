use syn::parse::discouraged::Speculative;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token;
use syn::{braced, bracketed};

/// Top-level macro input: `create!(Target, { fields })` or `create!(Target, [{ fields }, ...])`
pub(crate) struct CreateInput {
    pub target: Target,
    pub body: CreateItem,
}

/// What is being created
pub(crate) enum Target {
    /// Type path like `User` → generates `User::create()`
    Type(syn::Path),
    /// Expression like `user.todos()` → generates `expr.create()`
    Scope(syn::Expr),
}

/// A single `name: value` pair
pub(crate) struct FieldEntry {
    pub name: syn::Ident,
    /// Pre-computed `with_{name}` ident for the closure-based setter
    pub with_name: syn::Ident,
    pub value: CreateItem,
}

/// A recursive item in the create tree.
///
/// This unifies what was previously `Body` (at the root) and `FieldValue` (in fields)
/// into a single recursive type, since both represent the same structure.
pub(crate) enum CreateItem {
    /// Plain expression (literals, variables, etc.)
    Expr(Box<syn::Expr>),
    /// Anonymous struct literal: `{ field: value, ... }` — type inferred from context
    Single(Vec<FieldEntry>),
    /// Array of items: `[item, item, ...]`
    List(Vec<CreateItem>),
}

impl Parse for CreateInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let target = parse_target(input)?;
        input.parse::<syn::Token![,]>()?;
        let body = parse_create_item(input)?;
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

/// Parse a `CreateItem` from the token stream.
fn parse_create_item(input: ParseStream) -> syn::Result<CreateItem> {
    if input.peek(token::Brace) {
        // Anonymous struct literal: { field: value, ... }
        let fields = parse_braced_fields(input)?;
        Ok(CreateItem::Single(fields))
    } else if input.peek(token::Bracket) {
        // Array: [item, item, ...]
        let content;
        bracketed!(content in input);
        let items =
            Punctuated::<CreateItemInList, syn::Token![,]>::parse_terminated(&content)?;
        Ok(CreateItem::List(items.into_iter().map(|i| i.0).collect()))
    } else {
        // Plain expression — but guard against struct literals with a type prefix
        let expr = input.parse::<syn::Expr>()?;
        if let syn::Expr::Struct(ref s) = expr {
            let path = &s.path;
            return Err(syn::Error::new_spanned(
                path,
                format!(
                    "remove the type prefix `{}` — use `{{ ... }}` without a type name",
                    quote::quote!(#path)
                ),
            ));
        }
        Ok(CreateItem::Expr(Box::new(expr)))
    }
}

/// Wrapper for parsing items inside a list `[...]`
struct CreateItemInList(CreateItem);

impl Parse for CreateItemInList {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(CreateItemInList(parse_create_item(input)?))
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
        let with_name = syn::Ident::new(&format!("with_{}", name), name.span());
        input.parse::<syn::Token![:]>()?;
        let value = parse_create_item(input)?;
        Ok(FieldEntry { name, with_name, value })
    }
}

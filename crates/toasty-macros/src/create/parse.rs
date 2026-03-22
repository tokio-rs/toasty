use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token;
use syn::{braced, bracketed, parenthesized};

/// A recursive create item. Represents both the top-level macro input and
/// items nested inside a batch `[ ... ]`.
///
/// Four forms:
/// - `Path { fields }`        — type-target creation
/// - `in expr { fields }`     — scoped creation
/// - `Path::[ {..}, {..} ]`   — typed batch
/// - `[ Item, Item, ... ]`    — mixed batch (items are themselves `CreateItem`s)
pub(crate) enum CreateItem {
    /// `User { name: "Carl" }`
    Typed { path: syn::Path, fields: FieldSet },
    /// `in user.todos() { title: "buy milk" }`
    Scoped { expr: syn::Expr, fields: FieldSet },
    /// `User::[ { name: "Carl" }, { name: "Alice" } ]`
    TypedBatch {
        path: syn::Path,
        items: Vec<FieldSet>,
    },
    /// `[ User { ... }, Article::[ {...}, {...} ], in scope { ... } ]`
    Batch { items: Vec<CreateItem> },
    /// `( User { ... }, Article::[ {...}, {...} ] )`
    Tuple { items: Vec<CreateItem> },
}

/// A set of `name: value` field entries (i.e., the contents of `{ ... }`).
pub(crate) struct FieldSet(pub Vec<FieldEntry>);

/// A single `name: value` pair.
pub(crate) struct FieldEntry {
    pub name: syn::Ident,
    /// Pre-computed `with_{name}` ident for closure-based setters.
    pub with_name: syn::Ident,
    pub value: FieldValue,
}

/// The value side of a `name: value` field entry.
pub(crate) enum FieldValue {
    /// Plain expression (literals, variables, etc.)
    Expr(Box<syn::Expr>),
    /// Anonymous struct literal: `{ field: value, ... }` — type inferred from context
    Single(FieldSet),
    /// Array of items: `[item, item, ...]`
    List(Vec<FieldValue>),
}

/// Check if the next tokens are `::` followed by `[`.
///
/// We can't use `peek(Token![::]) && peek2(token::Bracket)` because `::` is
/// represented as two joint `:` punct tokens — `peek2` would see the second `:`
/// rather than the `[` that follows. Instead, fork the stream, consume `::`,
/// and then peek for `[`.
fn peek_path_sep_bracket(input: ParseStream) -> bool {
    if input.peek(syn::Token![::]) {
        let fork = input.fork();
        let _ = fork.parse::<syn::Token![::]>();
        fork.peek(token::Bracket)
    } else {
        false
    }
}

impl Parse for CreateItem {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(token::Paren) {
            // `( ... )` → tuple of items
            let content;
            parenthesized!(content in input);
            let items = Punctuated::<CreateItem, syn::Token![,]>::parse_terminated(&content)?;
            Ok(CreateItem::Tuple {
                items: items.into_iter().collect(),
            })
        } else if input.peek(token::Bracket) {
            // `[ ... ]` → batch of items
            let content;
            bracketed!(content in input);
            let items = Punctuated::<CreateItem, syn::Token![,]>::parse_terminated(&content)?;
            Ok(CreateItem::Batch {
                items: items.into_iter().collect(),
            })
        } else if input.peek(syn::Token![in]) {
            // `in expr { fields }` → scoped creation
            input.parse::<syn::Token![in]>()?;
            let expr = syn::Expr::parse_without_eager_brace(input)?;
            let fields = parse_braced_fields(input)?;
            Ok(CreateItem::Scoped { expr, fields })
        } else {
            // Must start with a path, then either `{` or `::[`.
            //
            // We can't use `syn::Path::parse` directly because it is greedy:
            // given `User::[`, it would consume the `::` as a path separator
            // and then fail when it finds `[` instead of an identifier.
            //
            // Instead, `parse_path_before_bracket` parses path segments but
            // stops before consuming `::` when it is followed by `[`.
            let path = parse_path_before_bracket(input)?;

            if input.peek(token::Brace) {
                // `Path { fields }` → typed creation
                let fields = parse_braced_fields(input)?;
                Ok(CreateItem::Typed { path, fields })
            } else if peek_path_sep_bracket(input) {
                // `Path::[ items ]` → typed batch
                input.parse::<syn::Token![::]>()?;
                let content;
                bracketed!(content in input);
                let items = Punctuated::<FieldSet, syn::Token![,]>::parse_terminated(&content)?;
                Ok(CreateItem::TypedBatch {
                    path,
                    items: items.into_iter().collect(),
                })
            } else {
                Err(input.error(
                    "expected `{` for single creation or `::[` for batch creation after type path",
                ))
            }
        }
    }
}

/// Parse a `syn::Path` but stop before consuming a `::` that is followed by `[`.
///
/// Standard `syn::Path::parse` greedily consumes `::` as a path separator, which
/// fails on `User::[` because `[` is not a valid path segment. This function
/// handles that by checking what follows each `::` before consuming it.
fn parse_path_before_bracket(input: ParseStream) -> syn::Result<syn::Path> {
    let leading_colon = if input.peek(syn::Token![::]) {
        Some(input.parse()?)
    } else {
        None
    };

    let mut segments = Punctuated::new();
    segments.push_value(syn::PathSegment::from(input.parse::<syn::Ident>()?));

    while input.peek(syn::Token![::]) {
        if peek_path_sep_bracket(input) {
            // `:: [` — this is the batch separator, not a path separator.
            break;
        }
        segments.push_punct(input.parse::<syn::Token![::]>()?);
        segments.push_value(syn::PathSegment::from(input.parse::<syn::Ident>()?));
    }

    Ok(syn::Path {
        leading_colon,
        segments,
    })
}

/// Parse `{ field: value, field: value, ... }` into a `FieldSet`.
fn parse_braced_fields(input: ParseStream) -> syn::Result<FieldSet> {
    let content;
    braced!(content in input);
    let entries = Punctuated::<FieldEntry, syn::Token![,]>::parse_terminated(&content)?;
    Ok(FieldSet(entries.into_iter().collect()))
}

impl Parse for FieldSet {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        parse_braced_fields(input)
    }
}

impl Parse for FieldEntry {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name = input.parse::<syn::Ident>()?;
        let with_name = syn::Ident::new(&format!("with_{}", name), name.span());
        input.parse::<syn::Token![:]>()?;
        let value = input.parse()?;
        Ok(FieldEntry {
            name,
            with_name,
            value,
        })
    }
}

impl Parse for FieldValue {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(token::Brace) {
            // Nested struct literal: `{ field: value, ... }`
            let fields = parse_braced_fields(input)?;
            Ok(FieldValue::Single(fields))
        } else if input.peek(token::Bracket) {
            // Nested list: `[item, item, ...]`
            let content;
            bracketed!(content in input);
            let items = Punctuated::<FieldValue, syn::Token![,]>::parse_terminated(&content)?;
            Ok(FieldValue::List(items.into_iter().collect()))
        } else {
            // Plain expression — guard against struct literals with a type prefix
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
            Ok(FieldValue::Expr(Box::new(expr)))
        }
    }
}

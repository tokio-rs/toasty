use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token;
use syn::{braced, bracketed};

/// The parsed `update!` invocation.
///
/// `update!(target_expr { field_entry, ... })`
///
/// The target is any Rust expression you can call `.update()` on — a
/// model instance (`&mut user`), a query builder
/// (`User::filter_by_id(id)`), or a scoped query
/// (`user.todos().filter_by_done(false)`).
pub(crate) struct UpdateItem {
    pub target: syn::Expr,
    pub fields: FieldSet,
}

/// A set of field entries — the contents of the `{ ... }` block.
pub(crate) struct FieldSet(pub Vec<FieldEntry>);

/// A single field entry. The macro accepts four shapes:
///
/// - `name: value` — explicit value (set semantics by default)
/// - `name` — shorthand for `name: name`
/// - `name.method(args)` — method-call shorthand for
///   `name: stmt::method(args)`
pub(crate) struct FieldEntry {
    pub name: syn::Ident,
    pub value: FieldValue,
}

/// The right-hand side of a field entry.
pub(crate) enum FieldValue {
    /// Plain expression: literals, variables, function calls,
    /// `stmt::*` builders the user spelled out.
    Expr(Box<syn::Expr>),
    /// Anonymous struct literal `{ sub: val, ... }` — for embedded-type
    /// partial updates. Expands to a `stmt::apply([stmt::patch(...)])`
    /// chain.
    Single(FieldSet),
    /// Array literal `[item, item, ...]`. Each item is itself a
    /// `FieldValue`. For has-many fields, `Single` items become
    /// `stmt::insert(...)` and the whole list is wrapped in
    /// `stmt::apply([...])`. For lists of plain `Expr` items the array
    /// passes through unchanged (set semantics, same as the chain form).
    List(Vec<FieldValue>),
    /// Method-call shorthand `name.method(args)` parsed from a field
    /// entry without an explicit value. The `combinator` ident names a
    /// function in the `toasty::stmt` module.
    Method {
        combinator: syn::Ident,
        args: Punctuated<syn::Expr, syn::Token![,]>,
    },
}

impl Parse for UpdateItem {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Parse the target expression. Use `parse_without_eager_brace` so
        // the parser doesn't eat `{ fields }` as a block expression — the
        // brace block belongs to the macro, not the target expression.
        let target = syn::Expr::parse_without_eager_brace(input)?;
        let fields = parse_braced_fields(input)?;
        Ok(UpdateItem { target, fields })
    }
}

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

        // Decide what follows the field name. Each shape has a distinct
        // lookahead token so there is no ambiguity:
        //
        //   `name: value`       → `:`
        //   `name`              → `,` or `}` (end-of-entry)
        //   `name.method(args)` → `.`
        if input.peek(syn::Token![:]) && !input.peek(syn::Token![::]) {
            // `name: value`
            input.parse::<syn::Token![:]>()?;
            let value = input.parse()?;
            Ok(FieldEntry { name, value })
        } else if input.peek(syn::Token![.]) {
            // `name.method(args)` — method-call shorthand
            input.parse::<syn::Token![.]>()?;
            let combinator = input.parse::<syn::Ident>()?;

            let args_content;
            syn::parenthesized!(args_content in input);
            let args = Punctuated::parse_terminated(&args_content)?;

            // The shorthand is one method call deep. Anything after the
            // closing paren that is not an entry terminator is an error.
            if !input.is_empty() && !input.peek(syn::Token![,]) && !input.peek(token::Brace) {
                return Err(input.error(
                    "method shorthand is one call deep — chained calls are not supported; \
                     use the explicit `field: expr` form for chained expressions",
                ));
            }

            Ok(FieldEntry {
                name,
                value: FieldValue::Method { combinator, args },
            })
        } else {
            // `name` shorthand — equivalent to `name: name`
            let expr = syn::Expr::Path(syn::ExprPath {
                attrs: vec![],
                qself: None,
                path: name.clone().into(),
            });
            Ok(FieldEntry {
                name,
                value: FieldValue::Expr(Box::new(expr)),
            })
        }
    }
}

impl Parse for FieldValue {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(token::Brace) {
            // Nested struct literal `{ sub: value, ... }` — partial
            // update of an embedded field.
            let fields = parse_braced_fields(input)?;
            Ok(FieldValue::Single(fields))
        } else if input.peek(token::Bracket) {
            // List literal `[item, item, ...]`. Items can themselves be
            // brace blocks (nested builders for has-many insert) or
            // plain expressions.
            let content;
            bracketed!(content in input);
            let items = Punctuated::<FieldValue, syn::Token![,]>::parse_terminated(&content)?;
            Ok(FieldValue::List(items.into_iter().collect()))
        } else {
            // Anything else is a plain Rust expression.
            let expr = input.parse::<syn::Expr>()?;
            Ok(FieldValue::Expr(Box::new(expr)))
        }
    }
}

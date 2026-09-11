use super::parse::{CreateItem, FieldEntry, FieldSet, FieldValue};

use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;

pub(crate) fn expand(item: &CreateItem) -> TokenStream {
    match item {
        CreateItem::Typed { path, fields } => {
            let span = path.span();
            let fields_path = quote_spanned! { span=> #path::fields() };
            let field_calls = expand_field_set(fields, &fields_path);

            quote_spanned! { span=>
                #path::create() #(#field_calls)*
            }
        }
        CreateItem::Scoped { expr, fields } => expand_scoped(expr, fields),
        CreateItem::TypedBatch { path, items } => {
            let batch = expand_typed_batch(path, items);
            quote! { toasty::batch(#batch) }
        }
        CreateItem::Tuple { items } => {
            let elements: Vec<_> = items.iter().map(expand_as_element).collect();
            quote! { toasty::batch(( #( #elements, )* )) }
        }
    }
}

/// Expand a scoped creation (`in expr { fields }`).
fn expand_scoped(expr: &syn::Expr, fields: &FieldSet) -> TokenStream {
    let span = expr.span();
    let fields_path = quote! { __scope_fields };
    let field_calls = expand_field_set(fields, &fields_path);

    let scope_fields_call =
        quote_spanned! { span=> toasty::codegen_support::scope_fields(&__scope) };
    let create_call = quote_spanned! { span=> toasty::codegen_support::create_in_scope(__scope) };

    quote! {
        {
            let __scope = #expr;
            let __scope_fields = #scope_fields_call;
            #create_call #(#field_calls)*
        }
    }
}

/// Expand a `CreateItem` as an element inside a tuple batch.
///
/// For `TypedBatch`, this produces a plain array `[b1, b2, ...]` (which
/// implements `IntoStatement` with `Returning = List<T>`) rather than
/// wrapping in `toasty::batch()`.
fn expand_as_element(item: &CreateItem) -> TokenStream {
    match item {
        CreateItem::TypedBatch { path, items } => expand_typed_batch(path, items),
        // All other forms expand identically to top-level.
        other => expand(other),
    }
}

/// Expand a `TypedBatch` into `[ builder1, builder2, ... ]`.
fn expand_typed_batch(path: &syn::Path, items: &[FieldSet]) -> TokenStream {
    let span = path.span();
    let fields_path = quote_spanned! { span=> #path::fields() };

    let builders: Vec<_> = items
        .iter()
        .map(|fields| {
            let field_calls = expand_field_set(fields, &fields_path);

            quote_spanned! { span=>
                #path::create() #(#field_calls)*
            }
        })
        .collect();
    quote! { [ #( #builders, )* ] }
}

/// Expand a `FieldSet` into method calls.
fn expand_field_set(fields: &FieldSet, path: &TokenStream) -> Vec<TokenStream> {
    fields.0.iter().map(|f| expand_field(f, path)).collect()
}

/// Expand a single field entry into a method call token stream.
fn expand_field(field: &FieldEntry, path: &TokenStream) -> TokenStream {
    let name = &field.name;
    let span = name.span();

    match &field.value {
        FieldValue::Expr(expr) => {
            let expr = expand_value(expr);
            quote_spanned! { span=> .#name(#expr) }
        }
        FieldValue::Single(sub_fields) => {
            let nested_path = quote! { #path.#name() };
            let sub_calls = expand_field_set(sub_fields, &nested_path);
            quote_spanned! { span=> .#name(#path.#name().create() #(#sub_calls)*) }
        }
        FieldValue::List(items) => {
            let nested_path = quote! { #path.#name() };
            let item_builders: Vec<_> = items
                .iter()
                .map(|item| expand_nested_item(item, path, name, span, &nested_path))
                .collect();
            quote_spanned! { span=> .#name([#(#item_builders),*]) }
        }
    }
}

pub(crate) fn expand_value(expr: &syn::Expr) -> TokenStream {
    expand_embedded_value(expr).unwrap_or_else(|| quote!(#expr))
}

fn expand_embedded_value(expr: &syn::Expr) -> Option<TokenStream> {
    if let syn::Expr::Paren(expr) = expr {
        return expand_embedded_value(&expr.expr);
    }
    if let syn::Expr::Call(call) = expr
        && let syn::Expr::Path(path) = &*call.func
        && path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "Some")
        && call.args.len() == 1
    {
        // Embedded builders implement IntoExpr<Option<Embed>> as well.
        return expand_embedded_value(&call.args[0]);
    }
    // Borrowed fields allow an embedded literal to omit its foreign keys.
    // Complete Rust values keep their normal IntoExpr encoding, including
    // unloaded Deferred slots and struct-update syntax.
    let syn::Expr::Struct(value) = expr else {
        return None;
    };
    if value.rest.is_some()
        || !value.fields.iter().any(|field| {
            matches!(field.expr, syn::Expr::Reference(_))
                || expand_embedded_value(&field.expr).is_some()
        })
    {
        return None;
    }
    let mut path = value.path.clone();
    let last = path.segments.last().unwrap().ident.clone();
    let is_variant = path.segments.len() > 1
        && path
            .segments
            .iter()
            .rev()
            .nth(1)
            .unwrap()
            .ident
            .to_string()
            .starts_with(char::is_uppercase);
    let constructor = if is_variant {
        path.segments.pop();
        path.segments.pop_punct();
        quote::format_ident!("__toasty_create_{}", last)
    } else {
        quote::format_ident!("__toasty_create")
    };
    let setters = value.fields.iter().map(|field| {
        let name = &field.member;
        let expr = expand_value(&field.expr);
        quote!(.#name(#expr))
    });
    Some(quote!(#path::#constructor() #(#setters)*))
}

/// Expand a single item within a field-level list using path-based builders.
///
/// `parent_path` is the current fields path (e.g., `User::fields()`).
/// `field_name` is the field identifier (e.g., `todos`).
/// `span` is the span of the field name for error reporting.
/// `nested_path` is `parent_path.field_name()` for deeper nesting.
fn expand_nested_item(
    item: &FieldValue,
    parent_path: &TokenStream,
    field_name: &syn::Ident,
    span: Span,
    nested_path: &TokenStream,
) -> TokenStream {
    match item {
        FieldValue::Single(fields) => {
            let sub_calls = expand_field_set(fields, nested_path);
            quote_spanned! { span=> #parent_path.#field_name().create() #(#sub_calls)* }
        }
        FieldValue::Expr(e) => {
            quote! { #e }
        }
        FieldValue::List(_) => {
            quote! { compile_error!("nested lists are not supported in create!") }
        }
    }
}

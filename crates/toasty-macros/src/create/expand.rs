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
            quote_spanned! { span=> #path::create() #(#field_calls)* }
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
///
/// Uses `toasty::codegen_support::scope_fields` to infer the scope type and
/// obtain its field struct for nested builders.
fn expand_scoped(expr: &syn::Expr, fields: &FieldSet) -> TokenStream {
    let span = expr.span();
    let fields_path = quote! { __scope_fields };
    let field_calls = expand_field_set(fields, &fields_path);

    // The `scope_fields` call is spanned to the user's expression so that
    // a missing `Scope` impl produces an error pointing at that expression.
    let scope_fields_call =
        quote_spanned! { span=> toasty::codegen_support::scope_fields(&__scope) };
    let create_call = quote_spanned! { span=> __scope.create() };

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
            quote_spanned! { span=> #path::create() #(#field_calls)* }
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

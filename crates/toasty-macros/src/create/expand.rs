use super::parse::{CreateItem, FieldEntry, FieldSet, FieldValue};

use proc_macro2::TokenStream;
use quote::quote;

pub(crate) fn expand(item: &CreateItem) -> TokenStream {
    match item {
        CreateItem::Typed { path, fields } => {
            let fields_path = quote! { #path::fields() };
            let field_calls = expand_field_set(fields, Some(&fields_path));
            quote! { #path::create() #(#field_calls)* }
        }
        CreateItem::Scoped { expr, fields } => {
            let field_calls = expand_field_set(fields, None);
            quote! { #expr.create() #(#field_calls)* }
        }
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
    let fields_path = quote! { #path::fields() };
    let builders: Vec<_> = items
        .iter()
        .map(|fields| {
            let field_calls = expand_field_set(fields, Some(&fields_path));
            quote! { #path::create() #(#field_calls)* }
        })
        .collect();
    quote! { [ #( #builders, )* ] }
}

/// Expand a `FieldSet` into method calls.
///
/// When `path` is `Some`, nested fields (Single/List) use path-based builders
/// instead of closure-based `CreateMany` patterns.
fn expand_field_set(fields: &FieldSet, path: Option<&TokenStream>) -> Vec<TokenStream> {
    fields.0.iter().map(|f| expand_field(f, path)).collect()
}

/// Expand a single field entry into a method call token stream.
fn expand_field(field: &FieldEntry, path: Option<&TokenStream>) -> TokenStream {
    let name = &field.name;
    let with_name = &field.with_name;

    match &field.value {
        FieldValue::Expr(expr) => {
            quote! { .#name(#expr) }
        }
        FieldValue::Single(sub_fields) => {
            if let Some(path) = path {
                // Path-based: use path.field().create() to build the nested item
                let nested_path = quote! { #path.#name() };
                let sub_calls = expand_field_set(sub_fields, Some(&nested_path));
                quote! { .#name(#path.#name().create() #(#sub_calls)*) }
            } else {
                // Fallback: closure-based approach
                let sub_calls = sub_fields.0.iter().map(|f| expand_field(f, None));
                quote! { .#with_name(|b| { #(let b = b #sub_calls;)* b }) }
            }
        }
        FieldValue::List(items) => {
            if let Some(path) = path {
                // Path-based: build an array of nested create builders
                let nested_path = quote! { #path.#name() };
                let item_builders: Vec<_> = items
                    .iter()
                    .map(|item| expand_nested_item_with_path(item, path, name, &nested_path))
                    .collect();
                quote! { .#name([#(#item_builders),*]) }
            } else {
                // Fallback: closure-based CreateMany approach
                let item_calls: Vec<_> = items.iter().map(expand_nested_item_fallback).collect();
                quote! { .#with_name(|b| b #(#item_calls)*) }
            }
        }
    }
}

/// Expand a single item within a field-level list using path-based builders.
///
/// `parent_path` is the current fields path (e.g., `User::fields()`).
/// `field_name` is the field identifier (e.g., `todos`).
/// `nested_path` is `parent_path.field_name()` for deeper nesting.
fn expand_nested_item_with_path(
    item: &FieldValue,
    parent_path: &TokenStream,
    field_name: &syn::Ident,
    nested_path: &TokenStream,
) -> TokenStream {
    match item {
        FieldValue::Single(fields) => {
            let sub_calls = expand_field_set(fields, Some(nested_path));
            quote! { #parent_path.#field_name().create() #(#sub_calls)* }
        }
        FieldValue::Expr(e) => {
            quote! { #e }
        }
        FieldValue::List(_) => {
            quote! { compile_error!("nested lists are not supported in create!") }
        }
    }
}

/// Fallback: expand a single item within a field-level list using CreateMany closures.
fn expand_nested_item_fallback(item: &FieldValue) -> TokenStream {
    match item {
        FieldValue::Single(fields) => {
            let sub_calls = fields.0.iter().map(|f| expand_field(f, None));
            quote! { .with_item(|b| { #(let b = b #sub_calls;)* b }) }
        }
        FieldValue::Expr(e) => {
            quote! { .item(#e) }
        }
        FieldValue::List(_) => {
            quote! { compile_error!("nested lists are not supported in create!") }
        }
    }
}

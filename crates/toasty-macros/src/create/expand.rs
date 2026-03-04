use super::parse::{CreateInput, CreateItem, FieldEntry, Target};

use proc_macro2::TokenStream;
use quote::quote;

pub(crate) fn expand(input: &CreateInput) -> TokenStream {
    let (single_init, many_init) = match &input.target {
        Target::Type(path) => (quote! { #path::create() }, quote! { #path::create_many() }),
        Target::Scope(expr) => (quote! { #expr.create() }, quote! { #expr.create_many() }),
    };

    match &input.body {
        CreateItem::Single(fields) => {
            let field_calls = expand_fields(fields);
            quote! { #single_init #(#field_calls)* }
        }
        CreateItem::List(items) => {
            let item_calls: Vec<_> = items.iter().map(expand_item).collect();
            quote! { #many_init #(#item_calls)* }
        }
        CreateItem::Expr(_) => {
            quote! { compile_error!("create! body must be a struct literal `{ ... }` or a list `[...]`") }
        }
    }
}

/// Expand a list of field entries into method calls.
fn expand_fields(fields: &[FieldEntry]) -> Vec<TokenStream> {
    fields.iter().map(expand_field).collect()
}

/// Expand a single field entry into a method call token stream.
fn expand_field(field: &FieldEntry) -> TokenStream {
    let name = &field.name;
    let with_name = &field.with_name;

    match &field.value {
        CreateItem::Expr(expr) => {
            quote! { .#name(#expr) }
        }
        CreateItem::Single(sub_fields) => {
            let sub_calls = sub_fields.iter().map(expand_field);
            quote! { .#with_name(|b| { #(let b = b #sub_calls;)* b }) }
        }
        CreateItem::List(items) => {
            let item_calls: Vec<_> = items.iter().map(expand_item).collect();
            quote! { .#with_name(|b| b #(#item_calls)*) }
        }
    }
}

/// Expand a single item within a list (at the root batch level or inside a
/// field-level `with_*` closure). Each item independently becomes:
///   Single  → `.with_item(|b| { sub_calls... b })`
///   Expr(e) → `.item(e)`  (e must implement IntoInsert)
///   List    → compile error (nested lists unsupported)
fn expand_item(item: &CreateItem) -> TokenStream {
    match item {
        CreateItem::Single(fields) => {
            let sub_calls = fields.iter().map(expand_field);
            quote! { .with_item(|b| { #(let b = b #sub_calls;)* b }) }
        }
        CreateItem::Expr(e) => {
            quote! { .item(#e) }
        }
        CreateItem::List(_) => {
            quote! { compile_error!("nested lists are not supported in create!") }
        }
    }
}

use super::parse::{CreateItem, FieldEntry, FieldSet, FieldValue};

use proc_macro2::TokenStream;
use quote::quote;

pub(crate) fn expand(item: &CreateItem) -> TokenStream {
    match item {
        CreateItem::Typed { path, fields } => {
            let field_calls = expand_field_set(fields);
            quote! { #path::create() #(#field_calls)* }
        }
        CreateItem::Scoped { expr, fields } => {
            let field_calls = expand_field_set(fields);
            quote! { #expr.create() #(#field_calls)* }
        }
        CreateItem::TypedBatch { path, items } => {
            let builders: Vec<_> = items
                .iter()
                .map(|fields| {
                    let field_calls = expand_field_set(fields);
                    quote! { #path::create() #(#field_calls)* }
                })
                .collect();
            quote! { ( #( #builders, )* ) }
        }
        CreateItem::Batch { items } => {
            let builders: Vec<_> = items.iter().map(expand).collect();
            quote! { ( #( #builders, )* ) }
        }
    }
}

/// Expand a `FieldSet` into method calls.
fn expand_field_set(fields: &FieldSet) -> Vec<TokenStream> {
    fields.0.iter().map(expand_field).collect()
}

/// Expand a single field entry into a method call token stream.
fn expand_field(field: &FieldEntry) -> TokenStream {
    let name = &field.name;
    let with_name = &field.with_name;

    match &field.value {
        FieldValue::Expr(expr) => {
            quote! { .#name(#expr) }
        }
        FieldValue::Single(sub_fields) => {
            let sub_calls = sub_fields.0.iter().map(expand_field);
            quote! { .#with_name(|b| { #(let b = b #sub_calls;)* b }) }
        }
        FieldValue::List(items) => {
            let item_calls: Vec<_> = items.iter().map(expand_nested_item).collect();
            quote! { .#with_name(|b| b #(#item_calls)*) }
        }
    }
}

/// Expand a single item within a field-level list (e.g., `todos: [{ ... }, { ... }]`).
fn expand_nested_item(item: &FieldValue) -> TokenStream {
    match item {
        FieldValue::Single(fields) => {
            let sub_calls = fields.0.iter().map(expand_field);
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

use super::parse::{CreateItem, FieldEntry, FieldSet, FieldValue};

use proc_macro2::TokenStream;
use quote::quote;

pub(crate) fn expand(item: &CreateItem) -> TokenStream {
    match item {
        CreateItem::Typed { path, fields } => {
            let verify = expand_verify_chain(Some(path), fields);
            let field_calls = expand_field_set(fields);
            quote! {
                {
                    #verify
                    #path::create() #(#field_calls)*
                }
            }
        }
        CreateItem::Scoped { expr, fields } => {
            let field_calls = expand_field_set(fields);
            quote! { #expr.create() #(#field_calls)* }
        }
        CreateItem::TypedBatch { path, items } => {
            let verifies: Vec<_> = items
                .iter()
                .map(|fields| expand_verify_chain(Some(path), fields))
                .collect();
            let builders: Vec<_> = items
                .iter()
                .map(|fields| {
                    let field_calls = expand_field_set(fields);
                    quote! { #path::create() #(#field_calls)* }
                })
                .collect();
            quote! {
                {
                    #( #verifies )*
                    ( #( #builders, )* )
                }
            }
        }
        CreateItem::Batch { items } => {
            let mut verifies = Vec::new();
            let mut builders = Vec::new();
            for item in items {
                collect_verifies(item, &mut verifies);
                builders.push(expand_builder_only(item));
            }
            quote! {
                {
                    #( #verifies )*
                    ( #( #builders, )* )
                }
            }
        }
    }
}

/// Expand a `CreateItem` into just the builder chain (no verification block).
/// Used inside mixed batch where verification is hoisted to the outer block.
fn expand_builder_only(item: &CreateItem) -> TokenStream {
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
            let builders: Vec<_> = items.iter().map(expand_builder_only).collect();
            quote! { ( #( #builders, )* ) }
        }
    }
}

/// Emit a verification chain for a type-target creation.
/// `path` is `None` for scoped creations (which skip verification).
fn expand_verify_chain(path: Option<&syn::Path>, fields: &FieldSet) -> TokenStream {
    let Some(path) = path else {
        return quote! {};
    };
    let verify_calls = expand_verify_field_set(fields);
    quote! { #path::__check_create(#path::__verify_create() #(#verify_calls)*); }
}

/// Convert a field set into verification method calls (no arguments).
fn expand_verify_field_set(fields: &FieldSet) -> Vec<TokenStream> {
    fields.0.iter().map(expand_verify_field).collect()
}

/// Convert a single field entry into a verification method call.
fn expand_verify_field(field: &FieldEntry) -> TokenStream {
    let name = &field.name;
    let with_name = &field.with_name;

    match &field.value {
        FieldValue::Expr(_) => {
            quote! { .#name() }
        }
        FieldValue::Single(_) | FieldValue::List(_) => {
            // Nested struct/list: use the with_ variant as identity
            quote! { .#with_name() }
        }
    }
}

/// Collect verification chains from a `CreateItem` tree (for mixed batch).
fn collect_verifies(item: &CreateItem, out: &mut Vec<TokenStream>) {
    match item {
        CreateItem::Typed { path, fields } => {
            out.push(expand_verify_chain(Some(path), fields));
        }
        CreateItem::Scoped { .. } => {
            // No verification for scoped items
        }
        CreateItem::TypedBatch { path, items } => {
            for fields in items {
                out.push(expand_verify_chain(Some(path), fields));
            }
        }
        CreateItem::Batch { items } => {
            for item in items {
                collect_verifies(item, out);
            }
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

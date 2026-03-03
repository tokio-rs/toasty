use super::parse::{CreateInput, CreateItem, FieldEntry, Target};

use proc_macro2::TokenStream;
use quote::quote;

pub(crate) fn expand(input: &CreateInput) -> TokenStream {
    match (&input.target, &input.body) {
        (Target::Type(path), CreateItem::Single(fields)) => {
            let field_calls = expand_fields(fields);
            quote! { #path::create() #(#field_calls)* }
        }
        (Target::Scope(expr), CreateItem::Single(fields)) => {
            let field_calls = expand_fields(fields);
            quote! { #expr.create() #(#field_calls)* }
        }
        (Target::Type(path), CreateItem::List(items)) => {
            let item_calls: Vec<_> = items
                .iter()
                .map(|item| {
                    let CreateItem::Single(fields) = item else {
                        // Nested lists or plain exprs at batch root are not supported
                        return quote! {
                            compile_error!("batch create items must be struct literals `{ ... }`")
                        };
                    };
                    let field_calls = expand_fields(fields);
                    quote! { .item(#path::create() #(#field_calls)*) }
                })
                .collect();
            quote! { #path::create_many() #(#item_calls)* }
        }
        (Target::Scope(expr), CreateItem::List(items)) => {
            let item_calls: Vec<_> = items
                .iter()
                .map(|item| {
                    let CreateItem::Single(fields) = item else {
                        return quote! {
                            compile_error!("batch create items must be struct literals `{ ... }`")
                        };
                    };
                    let field_calls = expand_fields(fields);
                    quote! { .item({ let mut __b = Default::default(); #(__b = __b #field_calls;)* __b }) }
                })
                .collect();
            quote! { #expr.create_many() #(#item_calls)* }
        }
        (_, CreateItem::Expr(_)) => {
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
            // { ... } in field position → .with_name(|b| b.f1(v1).f2(v2))
            let sub_calls = sub_fields.iter().map(expand_field);
            quote! { .#with_name(|b| { #(let b = b #sub_calls;)* b }) }
        }
        CreateItem::List(items) => {
            // Each item is handled independently:
            //   Single  → .with_item(|b| { sub_calls... b })
            //   Expr(e) → .item(e)  (e must implement IntoInsert)
            //   List    → compile error (nested lists unsupported)
            let item_calls: Vec<_> = items
                .iter()
                .map(|item| match item {
                    CreateItem::Single(sub_fields) => {
                        let sub_calls = sub_fields.iter().map(expand_field);
                        quote! { .with_item(|b| { #(let b = b #sub_calls;)* b }) }
                    }
                    CreateItem::Expr(e) => {
                        quote! { .item(#e) }
                    }
                    CreateItem::List(_) => {
                        quote! { compile_error!("nested lists are not supported in create!") }
                    }
                })
                .collect();
            quote! { .#with_name(|b| b #(#item_calls)*) }
        }
    }
}

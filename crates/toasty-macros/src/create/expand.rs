use super::parse::{Body, CreateInput, FieldEntry, FieldValue, Target};

use proc_macro2::TokenStream;
use quote::quote;

pub(crate) fn expand(input: &CreateInput) -> TokenStream {
    match (&input.target, &input.body) {
        (Target::Type(path), Body::Single(fields)) => {
            let field_calls = expand_fields(fields);
            quote! { #path::create() #(#field_calls)* }
        }
        (Target::Scope(expr), Body::Single(fields)) => {
            let field_calls = expand_fields(fields);
            quote! { #expr.create() #(#field_calls)* }
        }
        (Target::Type(path), Body::Batch(items)) => {
            let item_calls: Vec<_> = items
                .iter()
                .map(|fields| {
                    let field_calls = expand_fields(fields);
                    quote! { .item(#path::create() #(#field_calls)*) }
                })
                .collect();
            quote! { #path::create_many() #(#item_calls)* }
        }
        (Target::Scope(expr), Body::Batch(items)) => {
            // Gap: scoped batch creation. We don't know the child type, so we
            // can't call `Type::create()` for each item. For now, generate
            // `expr.create_many()` and hope the items contain enough type info.
            //
            // This will likely need builder API changes to work properly.
            let item_calls: Vec<_> = items
                .iter()
                .map(|fields| {
                    let field_calls = expand_fields(fields);
                    // Without a type, we can't call Type::create(). Use a
                    // placeholder that will produce a compile error pointing
                    // to the gap.
                    quote! { .item({ let mut __b = Default::default(); #(__b = __b #field_calls;)* __b }) }
                })
                .collect();
            quote! { #expr.create_many() #(#item_calls)* }
        }
    }
}

/// Expand a list of field entries into method calls.
fn expand_fields(fields: &[FieldEntry]) -> Vec<TokenStream> {
    fields.iter().flat_map(expand_field).collect()
}

/// Expand a single field entry into a method call.
fn expand_field(field: &FieldEntry) -> Vec<TokenStream> {
    let name = &field.name;

    match &field.value {
        FieldValue::Expr(expr) => {
            vec![quote! { .#name(#expr) }]
        }
        FieldValue::Create { path, fields } => {
            let nested_calls = expand_fields(fields);
            vec![quote! { .#name(#path::create() #(#nested_calls)*) }]
        }
        FieldValue::List(values) => {
            // List values produce a single call passing a vec:
            //   todos: [Todo { title: "a" }, Todo { title: "b" }]
            //   → .todos(vec![Todo::create().title("a"), Todo::create().title("b")])
            //
            // We use vec! instead of an array because the IntoExpr<[T]> impl
            // for arrays calls by_ref() which isn't implemented on create builders.
            // Vec's impl uses into_iter() which calls into_expr() by value.
            let items: Vec<_> = values.iter().map(expand_value).collect();
            vec![quote! { .#name(vec![#(#items),*]) }]
        }
    }
}

/// Expand a field value into an expression (no method call wrapper).
fn expand_value(value: &FieldValue) -> TokenStream {
    match value {
        FieldValue::Expr(expr) => {
            quote! { #expr }
        }
        FieldValue::Create { path, fields } => {
            let nested_calls = expand_fields(fields);
            quote! { #path::create() #(#nested_calls)* }
        }
        FieldValue::List(values) => {
            let items: Vec<_> = values.iter().map(expand_value).collect();
            quote! { vec![#(#items),*] }
        }
    }
}

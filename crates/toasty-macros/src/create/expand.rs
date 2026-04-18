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

            // Validate the target model's fields directly against its CREATE_META
            // (the target type is known here, no monomorphization needed).
            let assertion = expand_typed_assertion(path, fields, span);
            let nested_assertions = expand_nested_assertions(fields, &fields_path);

            quote_spanned! { span=>
                {
                    #assertion
                    #nested_assertions
                    #path::create() #(#field_calls)*
                }
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
///
/// Uses `toasty::codegen_support::scope_fields` to infer the scope type and
/// obtain its field struct for nested builders. Also monomorphizes a
/// `ValidateCreate`-bounded helper on the scope expression so the scope's
/// target model is validated at compile time.
fn expand_scoped(expr: &syn::Expr, fields: &FieldSet) -> TokenStream {
    let span = expr.span();
    let fields_path = quote! { __scope_fields };
    let field_calls = expand_field_set(fields, &fields_path);
    let provided = provided_field_names(fields);
    let nested_assertions = expand_nested_assertions(fields, &fields_path);

    // The `scope_fields` call is spanned to the user's expression so that
    // a missing `Scope` impl produces an error pointing at that expression.
    let scope_fields_call =
        quote_spanned! { span=> toasty::codegen_support::scope_fields(&__scope) };
    let create_call = quote_spanned! { span=> __scope.create() };
    let check_block = expand_monomorphized_assertion(&quote! { __scope }, &provided, span);

    quote! {
        {
            let __scope = #expr;
            #check_block
            let __scope_fields = #scope_fields_call;
            #nested_assertions
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
            let assertion = expand_typed_assertion(path, fields, span);
            let nested_assertions = expand_nested_assertions(fields, &fields_path);
            let field_calls = expand_field_set(fields, &fields_path);
            quote_spanned! { span=>
                {
                    #assertion
                    #nested_assertions
                    #path::create() #(#field_calls)*
                }
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

// ===========================================================================
// Static assertions (`CreateMeta` validation)
//
// The helpers below emit compile-time assertions that every required field
// of the target model has been specified. There are two forms:
//
// - Typed: the target path is known (e.g. `User { ... }`), so we can
//   emit a plain `const _: () = assert_create_fields(...);`.
// - Monomorphized: the target type is only visible through an expression
//   (e.g. `user.todos()` or `User::fields().todos()`), so we introduce a
//   generic helper bounded on `ValidateCreate` and force monomorphization
//   by passing the expression to it. The compiler evaluates the const
//   expression inside the generic impl at monomorphization time.
// ===========================================================================

/// Collect the field names provided in a `FieldSet` (as literal strings).
fn provided_field_names(fields: &FieldSet) -> Vec<String> {
    fields.0.iter().map(|f| f.name.to_string()).collect()
}

/// Emit a direct const assertion when the target type is known.
fn expand_typed_assertion(path: &syn::Path, fields: &FieldSet, span: Span) -> TokenStream {
    let provided = provided_field_names(fields);
    let provided_tokens = provided.iter().map(|n| quote! { #n });

    quote_spanned! { span=>
        const _: () = toasty::codegen_support::assert_create_fields(
            &<#path as toasty::codegen_support::Model>::CREATE_META,
            &[ #( #provided_tokens ),* ],
        );
    }
}

/// Emit a monomorphization-based assertion bounded on `ValidateCreate`.
///
/// `expr_tokens` evaluates to a reference-worthy value that implements
/// `ValidateCreate` (e.g. a fields struct or a relation scope type). The
/// resulting block forces evaluation of a generic `const` that asserts
/// every required field on the target model is listed in `provided`.
fn expand_monomorphized_assertion(
    expr_tokens: &TokenStream,
    provided: &[String],
    span: Span,
) -> TokenStream {
    let provided_tokens = provided.iter().map(|n| quote! { #n });

    quote_spanned! { span=>
        {
            struct __ToastyCheck<__S: toasty::codegen_support::ValidateCreate>(
                ::core::marker::PhantomData<__S>,
            );
            impl<__S: toasty::codegen_support::ValidateCreate> __ToastyCheck<__S> {
                const __ASSERT: () = toasty::codegen_support::assert_create_fields(
                    <__S as toasty::codegen_support::ValidateCreate>::CREATE_META,
                    &[ #( #provided_tokens ),* ],
                );
            }
            fn __toasty_force_check<__S: toasty::codegen_support::ValidateCreate>(_: &__S) {
                let _ = __ToastyCheck::<__S>::__ASSERT;
            }
            __toasty_force_check(&#expr_tokens);
        }
    }
}

/// Walk a `FieldSet` and emit monomorphized assertions for every nested
/// create expression (`Single` or `List` field values).
///
/// `parent_path` is the token expression for the current fields struct
/// (e.g. `User::fields()` or `__scope_fields`). Nested accessors are built
/// by appending `.field_name()`.
fn expand_nested_assertions(fields: &FieldSet, parent_path: &TokenStream) -> TokenStream {
    let mut out = TokenStream::new();

    for entry in &fields.0 {
        let field_name = &entry.name;
        let span = field_name.span();
        let accessor = quote_spanned! { span=> #parent_path.#field_name() };

        match &entry.value {
            FieldValue::Expr(_) => {}
            FieldValue::Single(inner) => {
                let provided = provided_field_names(inner);
                let block = expand_monomorphized_assertion(&accessor, &provided, span);
                let deeper = expand_nested_assertions(inner, &accessor);
                out.extend(quote! {
                    #block
                    #deeper
                });
            }
            FieldValue::List(items) => {
                for item in items {
                    if let FieldValue::Single(inner) = item {
                        let provided = provided_field_names(inner);
                        let block = expand_monomorphized_assertion(&accessor, &provided, span);
                        let deeper = expand_nested_assertions(inner, &accessor);
                        out.extend(quote! {
                            #block
                            #deeper
                        });
                    }
                }
            }
        }
    }

    out
}

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

            // Collect field names for validation
            let field_names = field_name_strs(fields);
            let nested_checks = expand_nested_checks(fields, &fields_path);

            quote_spanned! { span=>
                {
                    const _CREATE: () =
                        #path::__check_create_fields(&[ #( #field_names ),* ]);
                    #nested_checks
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
/// Uses monomorphization-time const evaluation to validate the provided
/// fields against the scope's `ValidateCreate::CREATE_META`.
fn expand_scoped(expr: &syn::Expr, fields: &FieldSet) -> TokenStream {
    let span = expr.span();
    let fields_path = quote! { __scope_fields };
    let field_calls = expand_field_set(fields, &fields_path);
    let field_names = field_name_strs(fields);

    let scope_fields_call =
        quote_spanned! { span=> toasty::codegen_support::scope_fields(&__scope) };
    let create_call = quote_spanned! { span=> __scope.create() };

    let monomorphize_check = expand_monomorphize_check(&field_names);

    quote! {
        {
            let __scope = #expr;

            #monomorphize_check
            fn __force_check<__S: toasty::codegen_support::ValidateCreate>(_: &__S) {
                let _ = __Check::<__S>::__ASSERT;
            }
            __force_check(&__scope);

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
            let field_names = field_name_strs(fields);
            let nested_checks = expand_nested_checks(fields, &fields_path);

            quote_spanned! { span=>
                {
                    const _CREATE: () =
                        #path::__check_create_fields(&[ #( #field_names ),* ]);
                    #nested_checks
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

// === Validation helpers ===

/// Collect field name string literals from a `FieldSet`.
fn field_name_strs(fields: &FieldSet) -> Vec<String> {
    fields.0.iter().map(|f| f.name.to_string()).collect()
}

/// Generate a monomorphization-based const check struct and impl.
///
/// Uses `assert_create_fields` (a `const fn`) for the check. This produces
/// a generic error message because `const fn` on stable Rust cannot panic
/// with formatted strings. For typed creates, the macro calls the per-model
/// `__check_create_fields` method instead, which has field-specific messages.
fn expand_monomorphize_check(field_names: &[String]) -> TokenStream {
    quote! {
        struct __Check<__S: toasty::codegen_support::ValidateCreate>(
            std::marker::PhantomData<__S>,
        );
        impl<__S: toasty::codegen_support::ValidateCreate> __Check<__S> {
            const __ASSERT: () = toasty::codegen_support::assert_create_fields(
                __S::CREATE_META,
                &[ #( #field_names ),* ],
            );
        }
    }
}

/// Emit validation blocks for nested field values (Single and List items
/// that contain FieldSets).
///
/// For each field whose value is `Single(sub_fields)` or `List([Single(sub_fields), ...])`
/// this generates a monomorphization block that validates the nested fields.
fn expand_nested_checks(fields: &FieldSet, fields_path: &TokenStream) -> TokenStream {
    let checks: Vec<_> = fields
        .0
        .iter()
        .filter_map(|entry| expand_nested_check_for_entry(entry, fields_path))
        .collect();

    quote! { #( #checks )* }
}

/// Emit a validation block for a single field entry if it contains nested FieldSets.
fn expand_nested_check_for_entry(
    entry: &FieldEntry,
    fields_path: &TokenStream,
) -> Option<TokenStream> {
    let name = &entry.name;
    let nested_path = quote! { #fields_path.#name() };

    match &entry.value {
        FieldValue::Single(sub_fields) => {
            Some(expand_nested_check_for_sub_fields(sub_fields, &nested_path))
        }
        FieldValue::List(items) => {
            // Validate each FieldSet in the list. All items in a list target
            // the same model, so we validate each independently (they can have
            // different field subsets).
            let item_checks: Vec<_> = items
                .iter()
                .filter_map(|item| match item {
                    FieldValue::Single(sub_fields) => {
                        Some(expand_nested_check_for_sub_fields(sub_fields, &nested_path))
                    }
                    _ => None,
                })
                .collect();

            if item_checks.is_empty() {
                None
            } else {
                Some(quote! { #( #item_checks )* })
            }
        }
        FieldValue::Expr(_) => None,
    }
}

/// Emit a validation block for a `FieldSet` of sub-fields at `nested_path`,
/// recursively including checks for any deeper nested fields.
fn expand_nested_check_for_sub_fields(
    sub_fields: &FieldSet,
    nested_path: &TokenStream,
) -> TokenStream {
    let field_names = field_name_strs(sub_fields);
    let deeper = expand_nested_checks(sub_fields, nested_path);
    expand_nested_validation_block(nested_path, &field_names, deeper)
}

/// Generate a monomorphization-based validation block for a nested level.
///
/// `nested_expr` is an expression like `User::fields().todos()` whose type
/// implements `ValidateCreate`.
fn expand_nested_validation_block(
    nested_expr: &TokenStream,
    field_names: &[String],
    deeper_checks: TokenStream,
) -> TokenStream {
    quote! {
        {
            let __nested = #nested_expr;
            struct __Check<__S: toasty::codegen_support::ValidateCreate>(
                std::marker::PhantomData<__S>,
            );
            impl<__S: toasty::codegen_support::ValidateCreate> __Check<__S> {
                const __ASSERT: () = toasty::codegen_support::assert_create_fields(
                    __S::CREATE_META,
                    &[ #( #field_names ),* ],
                );
            }
            fn __force<__S: toasty::codegen_support::ValidateCreate>(_: &__S) {
                let _ = __Check::<__S>::__ASSERT;
            }
            __force(&__nested);
            #deeper_checks
        }
    }
}

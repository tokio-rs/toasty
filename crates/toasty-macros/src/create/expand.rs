use super::parse::{CreateItem, FieldEntry, FieldSet, FieldValue};

use proc_macro2::TokenStream;
use quote::quote;

/// Context for the current expansion, tracking the root model path and the
/// chain of relation field names traversed so far. Used to construct field-path
/// expressions like `<User>::fields().todos().tags()` for nested list creates.
struct ExpandCtx<'a> {
    /// The root model type path (e.g., `User`).
    model_path: &'a syn::Path,
    /// Sequence of relation field names traversed to reach the current point.
    field_chain: Vec<&'a syn::Ident>,
}

impl<'a> ExpandCtx<'a> {
    fn new(model_path: &'a syn::Path) -> Self {
        Self {
            model_path,
            field_chain: vec![],
        }
    }

    /// Return a new context with `field` appended to the chain.
    fn push(&self, field: &'a syn::Ident) -> Self {
        let mut chain = self.field_chain.clone();
        chain.push(field);
        Self {
            model_path: self.model_path,
            field_chain: chain,
        }
    }

    /// Build the field-path expression: `<Model>::fields().f1().f2()...`
    fn field_path_expr(&self) -> TokenStream {
        let model = self.model_path;
        let fields = &self.field_chain;
        quote! { <#model>::fields() #(.#fields())* }
    }
}

pub(crate) fn expand(item: &CreateItem) -> TokenStream {
    match item {
        CreateItem::Typed { path, fields } => {
            let ctx = ExpandCtx::new(path);
            let field_calls = expand_field_set_ctx(fields, &ctx);
            quote! { #path::create() #(#field_calls)* }
        }
        CreateItem::Scoped { expr, fields } => {
            let field_calls = expand_field_set(fields);
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
    let ctx = ExpandCtx::new(path);
    let builders: Vec<_> = items
        .iter()
        .map(|fields| {
            let field_calls = expand_field_set_ctx(fields, &ctx);
            quote! { #path::create() #(#field_calls)* }
        })
        .collect();
    quote! { [ #( #builders, )* ] }
}

// --- Context-aware expansion (used for Typed and TypedBatch) ---

/// Expand a `FieldSet` into method calls, using the field-path context for
/// nested list creates.
fn expand_field_set_ctx<'a>(fields: &'a FieldSet, ctx: &ExpandCtx<'a>) -> Vec<TokenStream> {
    fields.0.iter().map(|f| expand_field_ctx(f, ctx)).collect()
}

/// Expand a single field entry into a method call, using the field-path context
/// for nested list creates.
fn expand_field_ctx<'a>(field: &'a FieldEntry, ctx: &ExpandCtx<'a>) -> TokenStream {
    let name = &field.name;
    let with_name = &field.with_name;

    match &field.value {
        FieldValue::Expr(expr) => {
            quote! { .#name(#expr) }
        }
        FieldValue::Single(sub_fields) => {
            let child_ctx = ctx.push(name);
            let sub_calls: Vec<_> = sub_fields
                .0
                .iter()
                .map(|f| expand_field_ctx(f, &child_ctx))
                .collect();
            quote! { .#with_name(|b| { #(let b = b #sub_calls;)* b }) }
        }
        FieldValue::List(items) => {
            // Use the field path to create builders via the Create trait
            // instead of CreateMany.
            //
            // For `todos: [{title: "a"}, {title: "b"}]` on model `User`, this
            // expands to:
            //
            //   .todos({
            //       fn __builder<__F: toasty::Create>(_: &__F) -> __F::Builder {
            //           __F::builder()
            //       }
            //       let __field = <User>::fields().todos();
            //       [{ let b = __builder(&__field); let b = b.title("a"); b },
            //        { let b = __builder(&__field); let b = b.title("b"); b }]
            //   })
            let child_ctx = ctx.push(name);
            let field_path = child_ctx.field_path_expr();
            let item_exprs: Vec<_> = items
                .iter()
                .map(|item| expand_list_item_ctx(item, &child_ctx))
                .collect();
            quote! {
                .#name({
                    fn __builder<__F: toasty::Create>(_: &__F) -> __F::Builder {
                        __F::builder()
                    }
                    let __field = #field_path;
                    [ #(#item_exprs),* ]
                })
            }
        }
    }
}

/// Expand a single item within a field-level list, using the field-path context
/// to obtain a builder from the `Create` trait.
fn expand_list_item_ctx<'a>(item: &'a FieldValue, ctx: &ExpandCtx<'a>) -> TokenStream {
    match item {
        FieldValue::Single(fields) => {
            let sub_calls: Vec<_> = fields.0.iter().map(|f| expand_field_ctx(f, ctx)).collect();
            quote! { {
                let b = __builder(&__field);
                #(let b = b #sub_calls;)*
                b
            } }
        }
        FieldValue::Expr(e) => {
            quote! { #e }
        }
        FieldValue::List(_) => {
            quote! { compile_error!("nested lists are not supported in create!") }
        }
    }
}

// --- Legacy expansion without context (used for Scoped creates) ---

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

use super::parse::{FieldEntry, FieldSet, FieldValue, UpdateItem};

use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;

/// Collects user-supplied value expressions so they can be evaluated
/// before the target's `.update()` call.
///
/// Instance targets borrow `&mut target` for the whole builder chain, so
/// a value expression that reads the target (`done: !todo.done`) would
/// hit E0503 if evaluated inside the chain. Hoisting evaluates every
/// value first, while the target is still unborrowed.
///
/// The values are evaluated as a `match` scrutinee tuple rather than
/// `let` bindings: temporaries created in a scrutinee live for the whole
/// `match`, so expressions that borrow from temporaries
/// (`name: s.to_uppercase().as_str()`) keep compiling exactly as they
/// did when they were evaluated inline in the chain. Tuple evaluation is
/// left-to-right, preserving field-order evaluation.
#[derive(Default)]
struct Hoist {
    exprs: Vec<TokenStream>,
    idents: Vec<syn::Ident>,
}

impl Hoist {
    /// Register `expr` for pre-evaluation and return the `__value_N`
    /// ident that will be bound to it. Both carry the expression's span
    /// so type errors point at the user's code.
    fn hoist(&mut self, expr: &syn::Expr) -> TokenStream {
        let span = expr.span();
        let ident = format_ident!("__value_{}", self.idents.len(), span = span);
        self.exprs.push(quote_spanned! { span=> #expr });
        self.idents.push(ident.clone());
        quote!(#ident)
    }
}

/// Top-level entry point. Expand an `update!` invocation into the
/// equivalent update-builder method chain.
///
/// Field value expressions are evaluated first, as a `match` scrutinee,
/// before the target expression is borrowed. This lets values read the
/// target (`update!(todo { done: !todo.done })`) without tripping over
/// the `&mut` borrow the builder chain holds.
///
/// The target expression is evaluated once at `.update()`, not at
/// some earlier `let` binding. This is what lets instance updates use
/// a bare identifier: `update!(user { ... })` expands to
/// `user.update()`, which auto-borrows `&mut user` the same way the
/// chain form does, leaving the original binding owned. Query
/// targets pass straight through and are consumed.
///
/// The fields struct needed for embedded-patch paths is recovered
/// from the bound update builder via the macro-only
/// `__macro_fields_root()` method.
///
/// Field-name validation falls out of the chain expansion: each named
/// field becomes a method call on the update builder, so a typo
/// surfaces as a "no method" error at the macro call site without the
/// macro doing anything specific.
pub(crate) fn expand(item: &UpdateItem) -> TokenStream {
    let target = &item.target;
    let target_span = target.span();

    let fields_path = quote!(__fields);
    let mut hoist = Hoist::default();
    let field_calls = expand_field_set(&item.fields, &fields_path, &mut hoist);

    let value_exprs = &hoist.exprs;
    let value_idents = &hoist.idents;

    // Trailing commas keep the tuple shape valid for zero and one
    // hoisted values.
    quote_spanned! { target_span=>
        match ( #( #value_exprs, )* ) {
            ( #( #value_idents, )* ) => {
                let __update = #target.update();

                // Fields path used for embedded-patch path construction.
                // `unused_variables` is silenced because the update may
                // have only top-level scalar entries that don't need it.
                #[allow(unused_variables)]
                let #fields_path = __update.__macro_fields_root();

                __update #(#field_calls)*
            }
        }
    }
}

/// Expand a [`FieldSet`] into a list of method calls.
fn expand_field_set(
    fields: &FieldSet,
    parent_path: &TokenStream,
    hoist: &mut Hoist,
) -> Vec<TokenStream> {
    fields
        .0
        .iter()
        .map(|entry| expand_field_entry(entry, parent_path, hoist))
        .collect()
}

/// Expand a single field entry into a method-call token stream.
///
/// `parent_path` is the fields-path expression for the parent type
/// (e.g. `__target_fields` at the top level, or
/// `parent_path.embedded_field()` for a nested embedded entry).
fn expand_field_entry(
    entry: &FieldEntry,
    parent_path: &TokenStream,
    hoist: &mut Hoist,
) -> TokenStream {
    let name = &entry.name;
    let span = name.span();

    match &entry.value {
        FieldValue::Expr(expr) => {
            let value = hoist.hoist(expr);
            quote_spanned! { span=> .#name(#value) }
        }
        FieldValue::Method { combinator, args } => {
            let args = hoist_args(args, hoist);
            quote_spanned! { span=> .#name(toasty::stmt::#combinator(#(#args),*)) }
        }
        FieldValue::Single(sub_fields) => {
            // Embedded partial update — expand into
            // `stmt::apply([stmt::patch(<sub_root>.sub(), val), ...])`.
            let nested_path = quote_spanned! { span=> #parent_path.#name() };
            let patches = expand_patch_entries(sub_fields, &nested_path, hoist);
            quote_spanned! { span=>
                .#name(toasty::stmt::apply([ #( #patches ),* ]))
            }
        }
        FieldValue::List(items) => {
            // Two emission shapes, depending on whether any list item is
            // a `{ ... }` create-builder shorthand:
            //
            // - With a builder shorthand (only meaningful for has-many),
            //   wrap each builder in `stmt::insert(..)` and the whole
            //   list in `stmt::apply([..])` so the inserts compose with
            //   any plain `stmt::*` siblings:
            //
            //     update!(user {
            //         todos: [{ title: "x" }, stmt::remove(&old)],
            //     })
            //     // →
            //     // .todos(stmt::apply([
            //     //     stmt::insert(<child create>),
            //     //     stmt::remove(&old),
            //     // ]))
            //
            // - With every item a plain expression, pass the array
            //   straight through. For `Vec<scalar>` this hits the
            //   setter's `Assign<List<T>>` impl as set semantics:
            //
            //     update!(article { tags: ["a", "b"] })
            //     // → .tags(["a", "b"])
            let any_builder = items
                .iter()
                .any(|item| matches!(item, FieldValue::Single(_)));
            let nested_path = quote_spanned! { span=> #parent_path.#name() };

            if any_builder {
                let entries: Vec<_> = items
                    .iter()
                    .map(|item| expand_has_many_list_item(item, &nested_path, span, hoist))
                    .collect();
                quote_spanned! { span=>
                    .#name(toasty::stmt::apply([ #( #entries ),* ]))
                }
            } else {
                let entries: Vec<_> = items
                    .iter()
                    .map(|item| match item {
                        FieldValue::Expr(e) => hoist.hoist(e),
                        // Nested `[ [...] ]` is rejected by the macro.
                        FieldValue::List(_) => quote_spanned! { span=>
                            compile_error!("nested lists are not supported in update!")
                        },
                        // Single/Method don't appear here because the
                        // any_builder check above routed us to the other
                        // branch when a Single was present, and Method only
                        // appears as a top-level entry shape.
                        FieldValue::Single(_) | FieldValue::Method { .. } => unreachable!(),
                    })
                    .collect();
                quote_spanned! { span=>
                    .#name([ #( #entries ),* ])
                }
            }
        }
    }
}

/// Hoist each argument of a method-shorthand call.
fn hoist_args(
    args: &syn::punctuated::Punctuated<syn::Expr, syn::Token![,]>,
    hoist: &mut Hoist,
) -> Vec<TokenStream> {
    args.iter().map(|arg| hoist.hoist(arg)).collect()
}

/// Expand each entry of an embedded brace block into a
/// `stmt::patch(<rooted>.sub_field(), value)` invocation.
fn expand_patch_entries(
    fields: &FieldSet,
    parent_path: &TokenStream,
    hoist: &mut Hoist,
) -> Vec<TokenStream> {
    fields
        .0
        .iter()
        .map(|entry| expand_patch_entry(entry, parent_path, hoist))
        .collect()
}

/// Expand a single sub-entry of an embedded brace block.
fn expand_patch_entry(
    entry: &FieldEntry,
    parent_path: &TokenStream,
    hoist: &mut Hoist,
) -> TokenStream {
    let name = &entry.name;
    let span = name.span();

    // The patch path is rooted at the embedded type itself, obtained
    // via the `into_root()` inherent method on the parent fields path.
    let rooted = quote_spanned! { span=> #parent_path.into_root() };

    match &entry.value {
        FieldValue::Expr(expr) => {
            let value = hoist.hoist(expr);
            quote_spanned! { span=>
                toasty::stmt::patch(#rooted.#name(), #value)
            }
        }
        FieldValue::Method { combinator, args } => {
            let args = hoist_args(args, hoist);
            quote_spanned! { span=>
                toasty::stmt::patch(
                    #rooted.#name(),
                    toasty::stmt::#combinator(#(#args),*),
                )
            }
        }
        FieldValue::Single(sub_fields) => {
            // Nested embedded patch — recurse.
            let nested_path = quote_spanned! { span=> #parent_path.#name() };
            let inner_patches = expand_patch_entries(sub_fields, &nested_path, hoist);
            quote_spanned! { span=>
                toasty::stmt::patch(
                    #rooted.#name(),
                    toasty::stmt::apply([ #( #inner_patches ),* ]),
                )
            }
        }
        FieldValue::List(_) => {
            quote_spanned! { span=>
                compile_error!("list values are not supported inside embedded patch blocks")
            }
        }
    }
}

/// Expand a single item inside a has-many list literal.
///
/// A brace block builds a create builder and is wrapped in
/// `stmt::insert`. A plain expression passes through as a complete
/// `Assignment<List<T>>` value (e.g. `stmt::remove(&old)`).
fn expand_has_many_list_item(
    item: &FieldValue,
    parent_path: &TokenStream,
    span: proc_macro2::Span,
    hoist: &mut Hoist,
) -> TokenStream {
    match item {
        FieldValue::Single(fields) => {
            // Build a child create builder via the field-list struct's
            // `.create()` method. A typo in any setter name surfaces as
            // a "no method named …" error at the macro call site.
            let setters = expand_create_setters(fields, hoist);
            quote_spanned! { span=>
                toasty::stmt::insert(#parent_path.create() #(#setters)*)
            }
        }
        FieldValue::Expr(e) => hoist.hoist(e),
        FieldValue::List(_) => quote_spanned! { span=>
            compile_error!("nested lists are not supported in update!")
        },
        FieldValue::Method { .. } => quote_spanned! { span=>
            compile_error!("method shorthand is not supported as a list item")
        },
    }
}

/// Expand the setters for a create-builder list item. Same shape as
/// `expand_field_entry`, but limited to entries that have a
/// counterpart on a create builder. Deeper nesting inside a has-many
/// list item is rejected for v1.
fn expand_create_setters(fields: &FieldSet, hoist: &mut Hoist) -> Vec<TokenStream> {
    fields
        .0
        .iter()
        .map(|entry| {
            let name = &entry.name;
            let span = name.span();
            match &entry.value {
                FieldValue::Expr(expr) => {
                    let value = hoist.hoist(expr);
                    quote_spanned! { span=> .#name(#value) }
                }
                FieldValue::Method { combinator, args } => {
                    let args = hoist_args(args, hoist);
                    quote_spanned! { span=> .#name(toasty::stmt::#combinator(#(#args),*)) }
                }
                FieldValue::Single(_) | FieldValue::List(_) => quote_spanned! { span=>
                    .#name(compile_error!(
                        "deeper nesting inside a has-many list item is not supported in update!"
                    ))
                },
            }
        })
        .collect()
}

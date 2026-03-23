use super::parse::{CompareExpr, CompareOp, FieldPath, FilterExpr, QueryInput, Value};

use proc_macro2::TokenStream;
use quote::quote;

/// Expand a parsed `QueryInput` into the corresponding method-chain calls.
pub(crate) fn expand(input: &QueryInput) -> TokenStream {
    let source = &input.source;

    match &input.filter {
        Some(filter) => {
            let filter_expr = expand_filter(source, filter);
            quote! { #source::filter(#filter_expr) }
        }
        None => {
            quote! { #source::all() }
        }
    }
}

/// Recursively expand a filter expression tree into token stream.
fn expand_filter(source: &syn::Path, expr: &FilterExpr) -> TokenStream {
    match expr {
        FilterExpr::And(lhs, rhs) => {
            let lhs = expand_filter(source, lhs);
            let rhs = expand_filter(source, rhs);
            quote! { #lhs.and(#rhs) }
        }
        FilterExpr::Or(lhs, rhs) => {
            let lhs = expand_filter(source, lhs);
            let rhs = expand_filter(source, rhs);
            quote! { #lhs.or(#rhs) }
        }
        FilterExpr::Not(inner) => {
            let inner = expand_filter(source, inner);
            // Use .not() instead of `!` to avoid Rust precedence issues
            // where `!(expr).method()` binds as `!((expr).method())`.
            quote! { (#inner).not() }
        }
        FilterExpr::Compare(cmp) => expand_compare(source, cmp),
        FilterExpr::Paren(inner) => expand_filter(source, inner),
    }
}

/// Expand a single comparison: `.field op value` → `Source::fields().field().method(value)`.
fn expand_compare(source: &syn::Path, cmp: &CompareExpr) -> TokenStream {
    let field_expr = expand_field_path(source, &cmp.lhs);
    let value = expand_value(source, &cmp.rhs);
    let method = compare_op_method(cmp.op);

    quote! { #field_expr.#method(#value) }
}

/// Expand a dot-prefixed field path into `Source::fields().seg1().seg2()...`.
fn expand_field_path(source: &syn::Path, path: &FieldPath) -> TokenStream {
    let mut out = quote! { #source::fields() };
    for seg in &path.segments {
        out = quote! { #out.#seg() };
    }
    out
}

/// Expand a value (RHS of comparison).
fn expand_value(source: &syn::Path, value: &Value) -> TokenStream {
    match value {
        Value::Lit(lit) => quote! { #lit },
        Value::Var(ident) => quote! { #ident },
        Value::Expr(expr) => quote! { #expr },
        Value::Field(path) => expand_field_path(source, path),
    }
}

/// Map a `CompareOp` to the corresponding method name identifier.
fn compare_op_method(op: CompareOp) -> syn::Ident {
    let name = match op {
        CompareOp::Eq => "eq",
        CompareOp::Ne => "ne",
        CompareOp::Gt => "gt",
        CompareOp::Ge => "ge",
        CompareOp::Lt => "lt",
        CompareOp::Le => "le",
    };
    syn::Ident::new(name, proc_macro2::Span::call_site())
}

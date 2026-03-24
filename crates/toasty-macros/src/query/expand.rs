use super::parse::{
    CompareOpKind, Expr, ExprBinaryOp, FieldPath, OrderByClause, OrderDirection, PaginationExpr,
    QueryInput,
};

use proc_macro2::TokenStream;
use quote::quote;

/// Expand a parsed `QueryInput` into the corresponding method-chain calls.
pub(crate) fn expand(input: &QueryInput) -> TokenStream {
    let source = &input.source;

    let base = match &input.filter {
        Some(filter) => {
            let filter_expr = expand_filter(source, filter);
            quote! { #source::filter(#filter_expr) }
        }
        None => {
            quote! { #source::all() }
        }
    };

    // Chain builder methods onto the base expression.
    let mut out = base;

    if let Some(order_by) = &input.order_by {
        let order_expr = expand_order_by(source, order_by);
        out = quote! { #out.order_by(#order_expr) };
    }

    if let Some(limit) = &input.limit {
        let limit_expr = expand_pagination_expr(limit);
        out = quote! { #out.limit(#limit_expr) };
    }

    // offset must come after limit (the API requires it)
    if let Some(offset) = &input.offset {
        let offset_expr = expand_pagination_expr(offset);
        out = quote! { #out.offset(#offset_expr) };
    }

    out
}

/// Expand an ORDER BY clause: `.field ASC` → `Source::fields().field().asc()`.
fn expand_order_by(source: &syn::Path, clause: &OrderByClause) -> TokenStream {
    let field = expand_field_path(source, &clause.field);
    match clause.direction {
        OrderDirection::Asc => quote! { #field.asc() },
        OrderDirection::Desc => quote! { #field.desc() },
    }
}

/// Expand a pagination expression (LIMIT or OFFSET value).
fn expand_pagination_expr(expr: &PaginationExpr) -> TokenStream {
    match expr {
        PaginationExpr::Lit(lit) => quote! { #lit },
        PaginationExpr::Var(ident) => quote! { #ident },
        PaginationExpr::RustExpr(expr) => quote! { #expr },
    }
}

/// Recursively expand an expression tree into token stream.
fn expand_filter(source: &syn::Path, expr: &Expr) -> TokenStream {
    match expr {
        Expr::And(lhs, rhs) => {
            let lhs = expand_filter(source, lhs);
            let rhs = expand_filter(source, rhs);
            quote! { #lhs.and(#rhs) }
        }
        Expr::Or(lhs, rhs) => {
            let lhs = expand_filter(source, lhs);
            let rhs = expand_filter(source, rhs);
            quote! { #lhs.or(#rhs) }
        }
        Expr::Not(inner) => {
            let inner = expand_filter(source, inner);
            // Use .not() instead of `!` to avoid Rust precedence issues
            // where `!(expr).method()` binds as `!((expr).method())`.
            quote! { (#inner).not() }
        }
        Expr::BinaryOp(cmp) => expand_binary_op(source, cmp),
        Expr::Paren(inner) => expand_filter(source, inner),
        Expr::Field(path) => expand_field_path(source, path),
        Expr::Lit(lit) => quote! { #lit },
        Expr::Var(ident) => quote! { #ident },
        Expr::RustExpr(expr) => quote! { #expr },
    }
}

/// Expand a binary operation: `lhs op rhs` → `lhs_expanded.method(rhs_expanded)`.
fn expand_binary_op(source: &syn::Path, cmp: &ExprBinaryOp) -> TokenStream {
    let lhs = expand_filter(source, &cmp.lhs);
    let rhs = expand_filter(source, &cmp.rhs);
    let method = compare_op_method(cmp.op.kind);

    quote! { #lhs.#method(#rhs) }
}

/// Expand a dot-prefixed field path into `Source::fields().seg1().seg2()...`.
fn expand_field_path(source: &syn::Path, path: &FieldPath) -> TokenStream {
    let mut out = quote! { #source::fields() };
    for seg in &path.segments {
        out = quote! { #out.#seg() };
    }
    out
}

/// Map a `CompareOpKind` to the corresponding method name identifier.
fn compare_op_method(op: CompareOpKind) -> syn::Ident {
    let name = match op {
        CompareOpKind::Eq => "eq",
        CompareOpKind::Ne => "ne",
        CompareOpKind::Gt => "gt",
        CompareOpKind::Ge => "ge",
        CompareOpKind::Lt => "lt",
        CompareOpKind::Le => "le",
    };
    syn::Ident::new(name, proc_macro2::Span::call_site())
}

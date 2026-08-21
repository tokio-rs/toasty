//! Rewrites embedded-enum variant literals in `create!` / `update!` field
//! values into variant expression builder chains.

use proc_macro2::TokenStream;
use quote::quote_spanned;
use syn::spanned::Spanned;

/// If `expr` is a struct literal whose path names an enum variant
/// (`Enum::Variant { .. }`), rewrite it into the derive-generated variant
/// expression builder chain: `Enum::__expr_variant().field(value)...`.
///
/// The builder fills a relation's sibling key slot(s) from a parent model
/// value, which is what allows `Owner::Human { human: &alice }` — a literal
/// that plain Rust would reject (missing key field, mismatched relation
/// type) — as a `create!` / `update!` field value. Complete literals keep
/// their meaning: every field maps to the builder setter of the same name,
/// and an unloaded `Deferred` sets nothing. The builder panics when a
/// required field ends up unset — neither written in the literal nor
/// filled from a loaded relation value — restoring the exhaustiveness
/// check the rewrite takes away from Rust.
///
/// Returns `None` — leaving the expression untouched — unless the literal
/// syntactically names a variant: at least two path segments with the last
/// two both starting uppercase (`Owner::Human`, `mod_path::Owner::Human`).
/// A one-segment literal (`Config { .. }`) or a module-qualified struct
/// (`config::Settings { .. }`) is a plain value. Literals with a qualified
/// self, generic arguments, a functional-update base (`..default`), or
/// unnamed members also pass through untouched.
pub(crate) fn rewrite_variant_literal(expr: &syn::Expr) -> Option<TokenStream> {
    let syn::Expr::Struct(lit) = expr else {
        return None;
    };

    if lit.qself.is_some() || lit.rest.is_some() || lit.dot2_token.is_some() {
        return None;
    }

    let segments = &lit.path.segments;
    if segments.len() < 2 {
        return None;
    }

    let variant_segment = segments.last().unwrap();
    let enum_segment = &segments[segments.len() - 2];

    for segment in [variant_segment, enum_segment] {
        if !matches!(segment.arguments, syn::PathArguments::None)
            || !starts_uppercase(&segment.ident)
        {
            return None;
        }
    }

    let mut setters = Vec::with_capacity(lit.fields.len());
    for field_value in &lit.fields {
        let syn::Member::Named(name) = &field_value.member else {
            return None;
        };
        let value = &field_value.expr;
        setters.push(quote_spanned! { name.span()=> .#name(#value) });
    }

    // The enum's path is the literal's path minus the variant segment.
    let enum_path = syn::Path {
        leading_colon: lit.path.leading_colon,
        segments: segments.iter().take(segments.len() - 1).cloned().collect(),
    };

    let entry_ident = syn::Ident::new(
        &format!(
            "__expr_{}",
            to_snake_case(&variant_segment.ident.to_string())
        ),
        variant_segment.ident.span(),
    );

    Some(quote_spanned! { lit.path.span()=>
        #enum_path::#entry_ident() #( #setters )*
    })
}

fn starts_uppercase(ident: &syn::Ident) -> bool {
    ident
        .to_string()
        .chars()
        .next()
        .is_some_and(|c| c.is_uppercase())
}

/// Matches the derive's variant-accessor naming (`Name::from_ident`), which
/// snake-cases the variant identifier.
fn to_snake_case(s: &str) -> String {
    use heck::ToSnakeCase;
    s.to_snake_case()
}

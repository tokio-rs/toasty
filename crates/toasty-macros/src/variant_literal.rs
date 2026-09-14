//! Recognizes embedded-enum variant literals in `create!` / `update!` field
//! values and expands them into construction builder chains.

use crate::model::schema::Name;

use proc_macro2::TokenStream;
use quote::quote_spanned;
use syn::spanned::Spanned;

/// Rewrites `Enum::Variant { field: value }` into a chain on the enum's
/// `EmbedCreate` builder. Other expressions pass through `value` whole.
/// `create!` returns values unchanged; `update!` hoists each written value.
///
/// Recognition requires the last two path segments to start uppercase,
/// without generic arguments. Unqualified variants, qualified-self paths,
/// functional updates, and unnamed fields pass through unchanged.
pub(crate) fn expand_value(
    expr: &syn::Expr,
    mut value: impl FnMut(&syn::Expr) -> TokenStream,
) -> TokenStream {
    let syn::Expr::Struct(lit) = expr else {
        return value(expr);
    };
    let segments = &lit.path.segments;

    // Validate the whole literal before calling `value`: the update hoister
    // must not register individual fields if the literal passes through whole.
    if lit.qself.is_some()
        || lit.rest.is_some()
        || lit.dot2_token.is_some()
        || segments.len() < 2
        || segments.iter().rev().take(2).any(|segment| {
            !matches!(segment.arguments, syn::PathArguments::None)
                || !starts_uppercase(&segment.ident)
        })
        || lit
            .fields
            .iter()
            .any(|field| !matches!(field.member, syn::Member::Named(_)))
    {
        return value(expr);
    }

    let method = Name::from_ident(&segments.last().unwrap().ident).ident;
    let enum_path = syn::Path {
        leading_colon: lit.path.leading_colon,
        segments: segments.iter().take(segments.len() - 1).cloned().collect(),
    };
    let setters = lit.fields.iter().map(|field| {
        let name = &field.member;
        let value = value(&field.expr);
        quote_spanned! { name.span()=> .#name(#value) }
    });
    let builder = quote_spanned! { enum_path.span()=>
        <#enum_path as toasty::codegen_support::EmbedCreate>::create()
    };

    quote_spanned! { lit.path.span()=>
        #builder.#method() #( #setters )*
    }
}

fn starts_uppercase(ident: &syn::Ident) -> bool {
    ident
        .to_string()
        .chars()
        .next()
        .is_some_and(|c| c.is_uppercase())
}

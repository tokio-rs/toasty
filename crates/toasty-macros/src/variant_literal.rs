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
    // Validate the whole literal before calling `value`: the update hoister
    // must not register individual fields if the literal passes through whole.
    let Some(literal) = VariantLiteral::recognize(expr) else {
        return value(expr);
    };

    literal.expand(&mut value)
}

struct VariantLiteral<'a> {
    expr: &'a syn::ExprStruct,
    enum_path: syn::Path,
    builder_method: syn::Ident,
}

impl<'a> VariantLiteral<'a> {
    fn recognize(expr: &'a syn::Expr) -> Option<Self> {
        let syn::Expr::Struct(expr) = expr else {
            return None;
        };

        if expr.qself.is_some() {
            return None;
        }

        if expr.rest.is_some() || expr.dot2_token.is_some() {
            return None;
        }

        if expr
            .fields
            .iter()
            .any(|field| !matches!(field.member, syn::Member::Named(_)))
        {
            return None;
        }

        let (enum_path, variant) = split_variant_path(&expr.path)?;
        let builder_method = Name::from_ident(&variant).ident;

        Some(Self {
            expr,
            enum_path,
            builder_method,
        })
    }

    fn expand(&self, value: &mut impl FnMut(&syn::Expr) -> TokenStream) -> TokenStream {
        let enum_path = &self.enum_path;
        let method = &self.builder_method;
        let setters = self.expr.fields.iter().map(|field| {
            let name = &field.member;
            let value = value(&field.expr);
            quote_spanned! { name.span()=> .#name(#value) }
        });
        let builder = quote_spanned! { enum_path.span()=>
            <#enum_path as toasty::codegen_support::EmbedCreate>::create()
        };

        quote_spanned! { self.expr.path.span()=>
            #builder.#method() #( #setters )*
        }
    }
}

fn split_variant_path(path: &syn::Path) -> Option<(syn::Path, syn::Ident)> {
    let mut segments = path.segments.iter().rev();
    let variant = segments.next()?;
    let enum_name = segments.next()?;

    if !is_type_name(enum_name) || !is_type_name(variant) {
        return None;
    }

    let enum_path = syn::Path {
        leading_colon: path.leading_colon,
        segments: path
            .segments
            .iter()
            .take(path.segments.len() - 1)
            .cloned()
            .collect(),
    };

    Some((enum_path, variant.ident.clone()))
}

fn is_type_name(segment: &syn::PathSegment) -> bool {
    matches!(segment.arguments, syn::PathArguments::None)
        && segment
            .ident
            .to_string()
            .chars()
            .next()
            .is_some_and(|c| c.is_uppercase())
}

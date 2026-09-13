//! Recognizes embedded-enum variant literals in `create!` / `update!` field
//! values and expands them into construction builder chains obtained
//! through the destination field.

use crate::model::schema::Name;

use proc_macro2::{Span, TokenStream};
use quote::quote_spanned;
use syn::spanned::Spanned;

/// A field value written as a qualified variant literal
/// (`Enum::Variant { field: value, .. }`).
///
/// The literal expands to a chain on the derive-generated construction
/// builder, reached through the destination field's fields handle:
/// `<dest>.create().variant().field(value)...`. The builder fills a
/// relation's sibling key slot(s) from a parent model value, which is what
/// allows `Owner::Human { human: &alice }` — a literal that plain Rust would
/// reject (missing key field, mismatched relation type) — as a `create!` /
/// `update!` field value. Complete literals keep their meaning: every field
/// maps to the builder setter of the same name, and an unloaded `Deferred`
/// sets nothing. The builder panics when a required field ends up unset —
/// neither written in the literal nor filled from a loaded relation value —
/// restoring the exhaustiveness check the rewrite takes away from Rust.
///
/// The destination field selects the enum; the qualifier only names it.
/// Builder lookup never resolves the qualifier, so an imported or renamed
/// variant (`Human { .. }`) is not recognized and passes through as an
/// ordinary value. The qualifier is still checked: the expansion pins the
/// builder to `IntoExpr<Qualifier>`, so `Other::Human { .. }` written into
/// an `Owner` field fails to compile at `Other`.
pub(crate) struct VariantLiteral<'a> {
    /// The literal's path minus the variant segment: the enum qualifier.
    enum_path: syn::Path,
    variant: &'a syn::Ident,
    fields: Vec<(&'a syn::Ident, &'a syn::Expr)>,
    span: Span,
}

impl<'a> VariantLiteral<'a> {
    /// Returns `None` — leaving the expression to pass through untouched —
    /// unless the literal syntactically names a variant: at least two path
    /// segments with the last two both starting uppercase (`Owner::Human`,
    /// `mod_path::Owner::Human`). A one-segment literal (`Config { .. }`)
    /// or a module-qualified struct (`config::Settings { .. }`) is a plain
    /// value. Literals with a qualified self, generic arguments, a
    /// functional-update base (`..default`), or unnamed members also pass
    /// through untouched.
    pub(crate) fn parse(expr: &'a syn::Expr) -> Option<Self> {
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

        let mut fields = Vec::with_capacity(lit.fields.len());
        for field_value in &lit.fields {
            let syn::Member::Named(name) = &field_value.member else {
                return None;
            };
            fields.push((name, &field_value.expr));
        }

        let enum_path = syn::Path {
            leading_colon: lit.path.leading_colon,
            segments: segments.iter().take(segments.len() - 1).cloned().collect(),
        };

        Some(Self {
            enum_path,
            variant: &variant_segment.ident,
            fields,
            span: lit.path.span(),
        })
    }

    /// Expand into the builder chain rooted at `dest`, the fields-handle
    /// expression of the destination field (`Object::fields().owner()`).
    ///
    /// `value` maps each written field value to the tokens passed to its
    /// setter; `create!` passes the expression through, `update!` registers
    /// it with its hoister.
    pub(crate) fn expand(
        &self,
        dest: &TokenStream,
        mut value: impl FnMut(&syn::Expr) -> TokenStream,
    ) -> TokenStream {
        // The selector method is named like the variant's field accessor.
        let method = Name::from_ident(self.variant).ident;

        let setters = self.fields.iter().map(|(name, expr)| {
            let value = value(expr);
            quote_spanned! { name.span()=> .#name(#value) }
        });

        let builder = quote_spanned! { self.span=>
            #dest.create().#method() #( #setters )*
        };

        let enum_path = &self.enum_path;
        quote_spanned! { enum_path.span()=>
            toasty::codegen_support::variant_of::<#enum_path, _>(#builder)
        }
    }
}

fn starts_uppercase(ident: &syn::Ident) -> bool {
    ident
        .to_string()
        .chars()
        .next()
        .is_some_and(|c| c.is_uppercase())
}

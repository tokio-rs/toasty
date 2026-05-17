use super::Name;

#[derive(Debug)]
pub(crate) struct HasMany {
    /// Target type
    pub(crate) ty: syn::Type,

    /// Singular field name
    pub(crate) singular: Name,

    /// Field on target that the relation references
    pub(crate) pair: Option<syn::Ident>,

    /// Field-name segments of a `#[has_many(via = a.b)]` multi-step relation.
    pub(crate) via: Option<Vec<syn::Ident>>,

    pub(crate) span: proc_macro2::Span,
}

impl HasMany {
    pub(super) fn from_ast(
        attr: &syn::Attribute,
        name: &syn::Ident,
        ty: &syn::Type,
        span: proc_macro2::Span,
    ) -> syn::Result<Self> {
        let singular = Name::from_str(
            &pluralizer::pluralize(&name.to_string(), 1, false),
            name.span(),
        );
        let (pair, via) = parse_has_relation_attrs(attr)?;

        Ok(Self {
            ty: ty.clone(),
            singular,
            pair,
            via,
            span,
        })
    }
}

/// Parse the optional `pair = <ident>` and `via = <a.b.c>` payloads on a
/// `#[has_many(...)]` or `#[has_one(...)]` attribute.
///
/// `pair` disambiguates the paired `BelongsTo` field on the target model when
/// multiple `BelongsTo` fields there point at the source. `via` declares a
/// multi-step relation reached by following a path of existing relations. The
/// two are mutually exclusive — a `via` relation has no pair.
pub(super) fn parse_has_relation_attrs(
    attr: &syn::Attribute,
) -> syn::Result<(Option<syn::Ident>, Option<Vec<syn::Ident>>)> {
    let mut pair = None;
    let mut via = None;

    if let syn::Meta::List(_) = &attr.meta {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("pair") {
                let value = meta.value()?;
                pair = Some(value.parse()?);
                Ok(())
            } else if meta.path.is_ident("via") {
                let value = meta.value()?;
                let segments = syn::punctuated::Punctuated::<syn::Ident, syn::Token![.]>::parse_separated_nonempty(value)?;
                via = Some(segments.into_iter().collect());
                Ok(())
            } else {
                Err(syn::Error::new_spanned(
                    &meta.path,
                    "expected `pair` or `via` attribute",
                ))
            }
        })?;
    }

    if pair.is_some() && via.is_some() {
        return Err(syn::Error::new_spanned(
            attr,
            "`pair` and `via` cannot be combined: a `via` relation has no pair",
        ));
    }

    Ok((pair, via))
}

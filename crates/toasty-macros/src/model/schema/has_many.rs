use super::Name;

#[derive(Debug)]
pub(crate) struct HasMany {
    /// Target type
    pub(crate) ty: syn::Type,

    /// Singular field name
    pub(crate) singular: Name,

    /// Field on target that the relation references
    pub(crate) pair: Option<syn::Ident>,

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
        let pair = parse_pair_attr(attr)?;

        Ok(Self {
            ty: ty.clone(),
            singular,
            pair,
            span,
        })
    }
}

/// Parse the `pair = <ident>` payload on a `#[has_many(...)]` or
/// `#[has_one(...)]` attribute. Both relations use this mechanism to
/// disambiguate the paired `BelongsTo` field on the target model when
/// multiple `BelongsTo` fields there point at the source.
pub(super) fn parse_pair_attr(attr: &syn::Attribute) -> syn::Result<Option<syn::Ident>> {
    let mut pair = None;

    if let syn::Meta::List(_) = &attr.meta {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("pair") {
                let value = meta.value()?;
                pair = Some(value.parse()?);
                Ok(())
            } else {
                Err(syn::Error::new_spanned(
                    &meta.path,
                    "expected `pair` attribute",
                ))
            }
        })?;
    }

    Ok(pair)
}

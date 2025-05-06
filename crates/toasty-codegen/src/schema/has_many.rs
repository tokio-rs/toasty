use super::Name;

#[derive(Debug)]
pub(crate) struct HasMany {
    /// Target type
    pub(crate) ty: syn::Type,

    /// Singular field name
    pub(crate) singular: Name,

    /// Insert method ident
    pub(crate) insert_ident: syn::Ident,

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
        let mut pair = None;
        let singular = Name::from_str(&std_util::str::singularize(&name.to_string()), name.span());
        let insert_ident = syn::Ident::new(&format!("insert_{}", singular.ident), name.span());

        if let syn::Meta::List(_) = &attr.meta {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("pair") {
                    let value = meta.value()?;
                    pair = Some(value.parse()?);
                } else {
                    return Err(syn::Error::new_spanned(
                        &meta.path,
                        "expected `pair` attribute",
                    ));
                }

                Ok(())
            })?;
        }

        Ok(Self {
            ty: ty.clone(),
            singular,
            insert_ident,
            pair,
            span,
        })
    }
}

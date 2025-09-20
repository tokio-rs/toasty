use super::ForeignKeyField;

#[derive(Debug)]
pub(crate) struct BelongsTo {
    /// Target type
    pub(crate) ty: syn::Type,

    /// Foreign key
    pub(crate) foreign_key: Vec<ForeignKeyField>,
}

impl BelongsTo {
    pub(super) fn from_ast(
        attr: &syn::Attribute,
        ty: &syn::Type,
        names: &[syn::Ident],
    ) -> syn::Result<Self> {
        let mut fk_sources: Vec<syn::Ident> = vec![];
        let mut fk_targets: Vec<syn::Ident> = vec![];
        let mut foreign_key = vec![];

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("key") {
                let value = meta.value()?;
                fk_sources.push(value.parse()?);
            } else if meta.path.is_ident("references") {
                let value = meta.value()?;
                fk_targets.push(value.parse()?);
            } else {
                return Err(syn::Error::new_spanned(
                    &meta.path,
                    "expected `key` or `references`",
                ));
            }

            Ok(())
        })?;

        if fk_sources.len() != fk_targets.len() {
            return Err(syn::Error::new_spanned(
                attr,
                "number of `key` and `references` attributes must match",
            ));
        }

        if fk_sources.is_empty() {
            return Err(syn::Error::new_spanned(
                attr,
                "expected at least one `key` and `references` attribute",
            ));
        }

        let parts = fk_sources.into_iter().zip(fk_targets);

        for (source, target) in parts {
            let source = names
                .iter()
                .position(|name| name == &source)
                .ok_or_else(|| {
                    syn::Error::new_spanned(
                        &source,
                        format!("source field `{source}` not found in names"),
                    )
                })?;

            foreign_key.push(ForeignKeyField { source, target });
        }

        // let syn::Meta::List(list) = &attr.meta else {
        //     return Err(syn::Error::new_spanned(
        //         &attr.meta,
        //         "expected #[relation(key = <field>, references = <field>)]",
        //     ));
        // };

        Ok(Self {
            ty: ty.clone(),
            foreign_key,
        })
    }
}

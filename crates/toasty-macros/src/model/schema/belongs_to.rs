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
        let mut fk_sources: Option<Vec<syn::Ident>> = None;
        let mut fk_targets: Option<Vec<syn::Ident>> = None;

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("key") {
                if fk_sources.is_some() {
                    return Err(meta.error(
                        "`key` specified more than once; use `key = [a, b]` for composite foreign keys",
                    ));
                }
                fk_sources = Some(parse_idents(&meta)?);
                Ok(())
            } else if meta.path.is_ident("references") {
                if fk_targets.is_some() {
                    return Err(meta.error(
                        "`references` specified more than once; use `references = [a, b]` for composite foreign keys",
                    ));
                }
                fk_targets = Some(parse_idents(&meta)?);
                Ok(())
            } else {
                Err(meta.error("expected `key` or `references`"))
            }
        })?;

        let fk_sources = fk_sources
            .ok_or_else(|| syn::Error::new_spanned(attr, "missing `key = ...` attribute"))?;
        let fk_targets = fk_targets
            .ok_or_else(|| syn::Error::new_spanned(attr, "missing `references = ...` attribute"))?;

        if fk_sources.is_empty() || fk_targets.is_empty() {
            return Err(syn::Error::new_spanned(
                attr,
                "`key` and `references` must each name at least one field",
            ));
        }

        if fk_sources.len() != fk_targets.len() {
            return Err(syn::Error::new_spanned(
                attr,
                format!(
                    "`key` has {} field(s) but `references` has {} field(s); they must match",
                    fk_sources.len(),
                    fk_targets.len(),
                ),
            ));
        }

        let mut foreign_key = vec![];

        for (source, target) in fk_sources.into_iter().zip(fk_targets) {
            let source_idx = names
                .iter()
                .position(|name| name == &source)
                .ok_or_else(|| {
                    syn::Error::new_spanned(
                        &source,
                        format!("source field `{source}` not found in names"),
                    )
                })?;

            foreign_key.push(ForeignKeyField {
                source: source_idx,
                target,
            });
        }

        Ok(Self {
            ty: ty.clone(),
            foreign_key,
        })
    }
}

fn parse_idents(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<Vec<syn::Ident>> {
    let value = meta.value()?;

    if value.peek(syn::token::Bracket) {
        let content;
        syn::bracketed!(content in value);
        let punctuated =
            syn::punctuated::Punctuated::<syn::Ident, syn::Token![,]>::parse_terminated(&content)?;
        Ok(punctuated.into_iter().collect())
    } else {
        Ok(vec![value.parse::<syn::Ident>()?])
    }
}

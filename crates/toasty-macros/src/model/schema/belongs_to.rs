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
        field_name: &syn::Ident,
        ty: &syn::Type,
        names: &[syn::Ident],
    ) -> syn::Result<Self> {
        let mut fk_sources: Option<Vec<syn::Ident>> = None;
        let mut fk_targets: Option<Vec<syn::Ident>> = None;

        // `#[belongs_to]` with no arguments infers both `key` and `references`;
        // only parse nested meta when arguments are actually present.
        if !matches!(attr.meta, syn::Meta::Path(_)) {
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
        }

        // `key` defaults to `<field>_id` when omitted.
        let fk_sources = match fk_sources {
            Some(sources) => sources,
            None => vec![syn::Ident::new(
                &format!("{field_name}_id"),
                field_name.span(),
            )],
        };

        if fk_sources.is_empty() {
            return Err(syn::Error::new_spanned(
                attr,
                "`key` must name at least one field",
            ));
        }

        // `references` defaults to `id`, the conventional primary key. A
        // composite key cannot be inferred, so it must be spelled out.
        let fk_targets = match fk_targets {
            Some(targets) => {
                if targets.is_empty() {
                    return Err(syn::Error::new_spanned(
                        attr,
                        "`references` must name at least one field",
                    ));
                }

                if fk_sources.len() != targets.len() {
                    return Err(syn::Error::new_spanned(
                        attr,
                        format!(
                            "`key` has {} field(s) but `references` has {} field(s); they must match",
                            fk_sources.len(),
                            targets.len(),
                        ),
                    ));
                }

                targets
            }
            None => {
                if fk_sources.len() != 1 {
                    return Err(syn::Error::new_spanned(
                        attr,
                        "`references` cannot be inferred for a composite `key`; \
                         name the referenced fields with `references = [...]`",
                    ));
                }

                vec![syn::Ident::new("id", field_name.span())]
            }
        };

        let mut foreign_key = vec![];

        for (source, target) in fk_sources.into_iter().zip(fk_targets) {
            let source_idx = names
                .iter()
                .position(|name| name == &source)
                .ok_or_else(|| {
                    syn::Error::new_spanned(
                        &source,
                        format!(
                            "foreign key field `{source}` not found on the model; \
                             add the field or name it explicitly with `key = ...`"
                        ),
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

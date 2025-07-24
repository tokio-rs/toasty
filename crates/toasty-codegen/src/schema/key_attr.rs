#[derive(Debug, Default)]
pub(crate) struct KeyAttr {
    pub(crate) partition: Vec<syn::Ident>,
    pub(crate) local: Vec<syn::Ident>,
}

impl KeyAttr {
    pub(super) fn from_ast(attr: &syn::Attribute, names: &[syn::Ident]) -> syn::Result<Self> {
        let mut partition = vec![];
        let mut local = vec![];

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("partition") {
                let value = meta.value()?;
                let ident: syn::Ident = value.parse()?;

                if !names.contains(&ident) {
                    return Err(syn::Error::new_spanned(
                        &ident,
                        format!("unknown field `{ident}`"),
                    ));
                }

                partition.push(ident);
            } else if meta.path.is_ident("local") {
                let value = meta.value()?;
                let ident = value.parse()?;

                if !names.contains(&ident) {
                    return Err(syn::Error::new_spanned(
                        &ident,
                        format!("unknown field `{ident}`"),
                    ));
                }

                local.push(ident);
            } else {
                return Err(syn::Error::new_spanned(
                    &meta.path,
                    "expected `partition` or `local`",
                ));
            }

            Ok(())
        })?;

        if partition.is_empty() {
            return Err(syn::Error::new_spanned(
                attr,
                "expected at least one `partition` attribute",
            ));
        }

        if local.is_empty() {
            return Err(syn::Error::new_spanned(
                attr,
                "expected at least one `local` attribute",
            ));
        }

        Ok(Self { partition, local })
    }
}

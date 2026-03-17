#[derive(Debug, Default)]
pub(crate) struct KeyAttr {
    pub(crate) partition: Vec<syn::Ident>,
    pub(crate) local: Vec<syn::Ident>,
}

impl KeyAttr {
    pub(super) fn from_ast(attr: &syn::Attribute, names: &[syn::Ident]) -> syn::Result<Self> {
        let mut partition = vec![];
        let mut local = vec![];
        let mut simple_fields = vec![];
        let mut has_named = false;

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("partition") || meta.path.is_ident("local") {
                if !simple_fields.is_empty() {
                    return Err(syn::Error::new_spanned(
                        &meta.path,
                        "cannot mix field names with `partition`/`local` syntax",
                    ));
                }

                has_named = true;
                let value = meta.value()?;
                let ident: syn::Ident = value.parse()?;

                if !names.contains(&ident) {
                    return Err(syn::Error::new_spanned(
                        &ident,
                        format!("unknown field `{ident}`"),
                    ));
                }

                if meta.path.is_ident("partition") {
                    partition.push(ident);
                } else {
                    local.push(ident);
                }
            } else {
                if has_named {
                    return Err(syn::Error::new_spanned(
                        &meta.path,
                        "cannot mix field names with `partition`/`local` syntax",
                    ));
                }

                let ident = meta
                    .path
                    .get_ident()
                    .ok_or_else(|| syn::Error::new_spanned(&meta.path, "expected a field name"))?;

                if !names.contains(ident) {
                    return Err(syn::Error::new_spanned(
                        ident,
                        format!("unknown field `{ident}`"),
                    ));
                }

                simple_fields.push(ident.clone());
            }

            Ok(())
        })?;

        if !simple_fields.is_empty() {
            // Simple mode: all fields become partition keys
            partition = simple_fields;
        } else {
            // Named mode: require both partition and local
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
        }

        Ok(Self { partition, local })
    }
}

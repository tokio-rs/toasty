#[derive(Debug, Default)]
pub(crate) struct KeyAttr {
    pub(crate) partition: Vec<syn::Ident>,
    pub(crate) local: Vec<syn::Ident>,
    pub(crate) name: Option<String>,
}

impl KeyAttr {
    pub(super) fn from_ast(attr: &syn::Attribute, names: &[syn::Ident]) -> syn::Result<Self> {
        let mut partition = vec![];
        let mut local = vec![];
        let mut simple_fields = vec![];
        let mut name: Option<String> = None;
        let mut has_named = false;

        attr.parse_nested_meta(|meta| {
            // `name = "..."` — explicit override for the generated index name.
            // Disambiguates from a model field literally called `name` by
            // requiring the `=` token; bare `name` still parses as a field ref.
            if meta.path.is_ident("name") && meta.input.peek(syn::Token![=]) {
                if name.is_some() {
                    return Err(syn::Error::new_spanned(
                        &meta.path,
                        "`name` specified more than once",
                    ));
                }

                let value: syn::LitStr = meta.value()?.parse()?;
                let value_str = value.value();

                if value_str.is_empty() {
                    return Err(syn::Error::new_spanned(
                        &value,
                        "`name` must be a non-empty string",
                    ));
                }

                name = Some(value_str);
                return Ok(());
            }

            if meta.path.is_ident("partition") || meta.path.is_ident("local") {
                if !simple_fields.is_empty() {
                    return Err(syn::Error::new_spanned(
                        &meta.path,
                        "cannot mix field names with `partition`/`local` syntax",
                    ));
                }

                let is_partition = meta.path.is_ident("partition");
                let target = if is_partition {
                    &mut partition
                } else {
                    &mut local
                };

                if !target.is_empty() {
                    return Err(syn::Error::new_spanned(
                        &meta.path,
                        if is_partition {
                            "`partition` specified more than once; use `partition = [a, b]` for multiple fields"
                        } else {
                            "`local` specified more than once; use `local = [a, b]` for multiple fields"
                        },
                    ));
                }

                has_named = true;
                let value = meta.value()?;

                let idents: Vec<syn::Ident> = if value.peek(syn::token::Bracket) {
                    let content;
                    syn::bracketed!(content in value);
                    let punctuated = syn::punctuated::Punctuated::<syn::Ident, syn::Token![,]>::parse_terminated(&content)?;
                    punctuated.into_iter().collect()
                } else {
                    vec![value.parse::<syn::Ident>()?]
                };

                for ident in &idents {
                    if !names.contains(ident) {
                        return Err(syn::Error::new_spanned(
                            ident,
                            format!("unknown field `{ident}`"),
                        ));
                    }
                }

                *target = idents;
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
                    "expected at least one `partition` field",
                ));
            }

            if local.is_empty() {
                return Err(syn::Error::new_spanned(
                    attr,
                    "expected at least one `local` field",
                ));
            }
        }

        Ok(Self {
            partition,
            local,
            name,
        })
    }
}

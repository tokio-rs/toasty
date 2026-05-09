use super::{ErrorSet, KeyAttr};

#[derive(Debug, Default)]
pub(crate) struct ModelAttr {
    /// Primary key definition
    pub(crate) key: Option<KeyAttr>,

    /// Model-level secondary index definitions
    pub(crate) indices: Vec<KeyAttr>,

    /// Optional database table name to map the model to
    pub(crate) table: Option<syn::LitStr>,

    /// Struct-level `#[auto]` (embedded newtype only). Stored as the
    /// originating attribute so downstream code can span errors back to the
    /// user's source. Only the bare `#[auto]` form is accepted; the inner
    /// field's `Auto` impl supplies the strategy.
    pub(crate) auto: Option<syn::Attribute>,
}

impl ModelAttr {
    pub(super) fn populate_from_ast(
        &mut self,
        attrs: &Vec<syn::Attribute>,
        names: &[syn::Ident],
    ) -> syn::Result<()> {
        let mut errs = ErrorSet::new();

        for attr in attrs {
            if attr.path().is_ident("key") {
                if self.key.is_some() {
                    errs.push(syn::Error::new_spanned(attr, "duplicate #[key] attribute"));
                } else {
                    match KeyAttr::from_ast(attr, names) {
                        Ok(key_attr) => self.key = Some(key_attr),
                        Err(e) => errs.push(e),
                    }
                }
            } else if attr.path().is_ident("index") {
                match KeyAttr::from_ast(attr, names) {
                    Ok(index_attr) => self.indices.push(index_attr),
                    Err(e) => errs.push(e),
                }
            } else if attr.path().is_ident("auto") {
                if self.auto.is_some() {
                    errs.push(syn::Error::new_spanned(attr, "duplicate #[auto] attribute"));
                } else if !matches!(attr.meta, syn::Meta::Path(_)) {
                    // Struct-level `#[auto]` always proxies the strategy from
                    // the inner field's `Auto` impl. Specifying a strategy
                    // here would shadow that and is rejected.
                    errs.push(syn::Error::new_spanned(
                        attr,
                        "struct-level #[auto] does not take arguments; \
                         the strategy is taken from the inner field's `Auto` impl",
                    ));
                } else {
                    self.auto = Some(attr.clone());
                }
            } else if attr.path().is_ident("table") {
                if self.table.is_some() {
                    return Err(syn::Error::new_spanned(attr, "duplicate `table` attribute"));
                }

                let syn::Meta::NameValue(meta) = &attr.meta else {
                    return Err(syn::Error::new_spanned(
                        attr,
                        "expected `table = \"table_name\"`",
                    ));
                };

                let syn::Expr::Lit(lit) = &meta.value else {
                    return Err(syn::Error::new_spanned(
                        attr,
                        "expected `table = \"table_name\"`",
                    ));
                };

                let syn::Lit::Str(lit) = &lit.lit else {
                    return Err(syn::Error::new_spanned(
                        attr,
                        "expected `table = \"table_name\"`",
                    ));
                };

                self.table = Some(lit.clone());
            }
        }

        if let Some(err) = errs.collect() {
            return Err(err);
        }

        Ok(())
    }
}

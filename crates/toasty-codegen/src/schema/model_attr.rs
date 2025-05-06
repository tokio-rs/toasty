use super::{ErrorSet, KeyAttr};

#[derive(Debug, Default)]
pub(crate) struct ModelAttr {
    /// Primary key definition
    pub(crate) key: Option<KeyAttr>,

    /// Optional database table name to map the model to
    pub(crate) table: Option<syn::LitStr>,
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
                    self.key = Some(KeyAttr::from_ast(attr, names)?);
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

        Ok(())
    }
}

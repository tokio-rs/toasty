use super::{ErrorSet, KeyAttr};

#[derive(Debug, Default)]
pub(crate) struct ModelAttr {
    /// Primary key definition
    pub(crate) key: Option<KeyAttr>,

    /// Optional database table name to map the model to
    pub(crate) table: Option<syn::Ident>,
}

impl ModelAttr {
    pub(super) fn populate_from_ast(
        &mut self,
        attrs: &mut Vec<syn::Attribute>,
        names: &[syn::Ident],
    ) -> syn::Result<()> {
        let mut errs = ErrorSet::new();

        let mut i = 0;
        while i < attrs.len() {
            let attr = &attrs[i];

            if attr.path().is_ident("key") {
                if self.key.is_some() {
                    errs.push(syn::Error::new_spanned(attr, "duplicate #[key] attribute"));
                } else {
                    self.key = Some(KeyAttr::from_ast(attr, names)?);
                }
            } else {
                i += 1;
                continue;
            }

            attrs.remove(i);
        }

        Ok(())
    }
}

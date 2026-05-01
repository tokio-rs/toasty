use super::{ErrorSet, KeyAttr};

#[derive(Debug, Default)]
pub(crate) struct ModelAttr {
    /// Primary key definition
    pub(crate) key: Option<KeyAttr>,

    /// Model-level secondary index definitions
    pub(crate) indices: Vec<KeyAttr>,

    /// Optional database table name to map the model to
    pub(crate) table: Option<syn::LitStr>,

    /// Parent model type for item collection (single-table design).
    /// Set when `#[item_collection(ParentType)]` is present on the model.
    pub(crate) item_collection: Option<syn::Type>,
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
            } else if attr.path().is_ident("unique") {
                // A struct-level `#[unique(...)]` is a composite unique index. It
                // mirrors `#[index(...)]` (simple and partition/local modes, plus
                // `name = "..."`) but enforces uniqueness across the listed fields.
                match KeyAttr::from_ast(attr, names) {
                    Ok(mut index_attr) => {
                        index_attr.unique = true;
                        self.indices.push(index_attr);
                    }
                    Err(e) => errs.push(e),
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
            } else if attr.path().is_ident("item_collection") {
                if self.item_collection.is_some() {
                    errs.push(syn::Error::new_spanned(
                        attr,
                        "duplicate #[item_collection] attribute",
                    ));
                } else {
                    match attr.parse_args::<syn::Type>() {
                        Ok(ty) => self.item_collection = Some(ty),
                        Err(e) => errs.push(e),
                    }
                }
            }
        }

        if let Some(err) = errs.collect() {
            return Err(err);
        }

        Ok(())
    }
}

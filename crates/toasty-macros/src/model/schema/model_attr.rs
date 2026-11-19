use super::{ErrorSet, KeyAttr, validate_comment};

#[derive(Debug, Default)]
pub(crate) struct ModelAttr {
    /// Primary key definition
    pub(crate) key: Option<KeyAttr>,

    /// Model-level secondary index definitions
    pub(crate) indices: Vec<KeyAttr>,

    /// Optional database table name to map the model to
    pub(crate) table: Option<syn::LitStr>,

    /// Optional database table comment.
    pub(crate) comment: Option<syn::LitStr>,
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
                match TableAttr::from_ast(attr) {
                    Ok(table_attr) => {
                        if let Some(table) = table_attr.name {
                            if self.table.is_some() {
                                errs.push(syn::Error::new_spanned(attr, "duplicate table name"));
                            } else {
                                self.table = Some(table);
                            }
                        }

                        if let Some(comment) = table_attr.comment {
                            if self.comment.is_some() {
                                errs.push(syn::Error::new_spanned(attr, "duplicate table comment"));
                            } else {
                                self.comment = Some(comment);
                            }
                        }
                    }
                    Err(err) => errs.push(err),
                }
            }
        }

        if let Some(err) = errs.collect() {
            return Err(err);
        }

        Ok(())
    }
}

struct TableAttr {
    name: Option<syn::LitStr>,
    comment: Option<syn::LitStr>,
}

impl TableAttr {
    fn from_ast(attr: &syn::Attribute) -> syn::Result<Self> {
        match &attr.meta {
            syn::Meta::NameValue(meta) => {
                let syn::Expr::Lit(lit) = &meta.value else {
                    return Err(expected_table_attr(attr));
                };

                let syn::Lit::Str(lit) = &lit.lit else {
                    return Err(expected_table_attr(attr));
                };

                Ok(Self {
                    name: Some(lit.clone()),
                    comment: None,
                })
            }
            syn::Meta::List(_) => attr.parse_args(),
            syn::Meta::Path(_) => Err(expected_table_attr(attr)),
        }
    }
}

impl syn::parse::Parse for TableAttr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut result = Self {
            name: None,
            comment: None,
        };

        loop {
            let lookahead = input.lookahead1();

            if lookahead.peek(syn::LitStr) {
                if result.name.is_some() {
                    return Err(syn::Error::new(input.span(), "duplicate table name"));
                }
                result.name = Some(input.parse()?);
            } else if lookahead.peek(kw::name) {
                if result.name.is_some() {
                    return Err(syn::Error::new(input.span(), "duplicate table name"));
                }
                let _name_token: kw::name = input.parse()?;
                let _eq_token: syn::Token![=] = input.parse()?;
                result.name = Some(input.parse()?);
            } else if lookahead.peek(kw::comment) {
                if result.comment.is_some() {
                    return Err(syn::Error::new(input.span(), "duplicate table comment"));
                }
                let _comment_token: kw::comment = input.parse()?;
                let _eq_token: syn::Token![=] = input.parse()?;
                let lit: syn::LitStr = input.parse()?;
                result.comment = Some(validate_comment(lit)?);
            } else {
                return Err(lookahead.error());
            }

            if input.is_empty() {
                break;
            }
            let _comma_token: syn::Token![,] = input.parse()?;
        }

        Ok(result)
    }
}

fn expected_table_attr(attr: &syn::Attribute) -> syn::Error {
    syn::Error::new_spanned(
        attr,
        "expected `table = \"table_name\"` or `table(comment = \"text\")`",
    )
}

mod kw {
    syn::custom_keyword!(comment);
    syn::custom_keyword!(name);
}

#[derive(Debug)]
pub(crate) enum ColumnType {
    VarChar(usize),
}

impl ColumnType {
    pub(super) fn from_ast(attr: &syn::Attribute) -> syn::Result<ColumnType> {
        let mut ret = None;

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("varchar") {
                let content;
                syn::parenthesized!(content in meta.input);
                let lit = content.parse::<syn::LitInt>()?;
                let size = lit.base10_parse::<usize>()?;
                ret = Some(ColumnType::VarChar(size));
            } else {
                return Err(syn::Error::new_spanned(
                    &meta.path,
                    "unexpected database type",
                ));
            }

            Ok(())
        })?;

        if let Some(ty) = ret {
            Ok(ty)
        } else {
            Err(syn::Error::new_spanned(attr, "expected a column type"))
        }
    }
}

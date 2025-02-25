use super::Name;

#[derive(Debug)]
pub(crate) struct Field {
    /// Field name
    pub(crate) name: Name,

    /// Field type
    pub(crate) ty: syn::Type,
}

impl Field {
    pub(super) fn from_ast(field: &syn::Field) -> syn::Result<Field> {
        let Some(ident) = &field.ident else {
            return Err(syn::Error::new_spanned(field, "model fields must be named"));
        };

        Ok(Field {
            name: Name::from_ident(ident),
            ty: field.ty.clone(),
        })
    }
}

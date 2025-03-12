#[derive(Debug)]
pub(crate) struct HasOne {
    /// Target type
    pub(crate) ty: syn::Type,
}

impl HasOne {
    pub(super) fn from_ast(ty: &syn::Type) -> syn::Result<HasOne> {
        Ok(HasOne { ty: ty.clone() })
    }
}

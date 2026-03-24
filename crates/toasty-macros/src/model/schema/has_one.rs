#[derive(Debug)]
pub(crate) struct HasOne {
    /// Target type
    pub(crate) ty: syn::Type,

    pub(crate) span: proc_macro2::Span,
}

impl HasOne {
    pub(super) fn from_ast(ty: &syn::Type, span: proc_macro2::Span) -> syn::Result<Self> {
        Ok(Self {
            ty: ty.clone(),
            span,
        })
    }
}

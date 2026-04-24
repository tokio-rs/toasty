use super::has_many::parse_pair_attr;

#[derive(Debug)]
pub(crate) struct HasOne {
    /// Target type
    pub(crate) ty: syn::Type,

    /// Field on target that the relation references
    pub(crate) pair: Option<syn::Ident>,

    pub(crate) span: proc_macro2::Span,
}

impl HasOne {
    pub(super) fn from_ast(
        attr: &syn::Attribute,
        ty: &syn::Type,
        span: proc_macro2::Span,
    ) -> syn::Result<Self> {
        Ok(Self {
            ty: ty.clone(),
            pair: parse_pair_attr(attr)?,
            span,
        })
    }
}

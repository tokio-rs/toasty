use super::has_many::parse_has_relation_attrs;

#[derive(Debug)]
pub(crate) struct HasOne {
    /// Target type
    pub(crate) ty: syn::Type,

    /// Field on target that the relation references
    pub(crate) pair: Option<syn::Ident>,

    /// Field-name segments of a `#[has_one(via = a.b)]` multi-step relation.
    pub(crate) via: Option<Vec<syn::Ident>>,

    pub(crate) span: proc_macro2::Span,
}

impl HasOne {
    pub(super) fn from_ast(
        attr: &syn::Attribute,
        ty: &syn::Type,
        span: proc_macro2::Span,
    ) -> syn::Result<Self> {
        let (pair, via) = parse_has_relation_attrs(attr)?;

        Ok(Self {
            ty: ty.clone(),
            pair,
            via,
            span,
        })
    }
}

use super::has_many::{RelationAttrs, parse_has_relation_attrs};

#[derive(Debug)]
pub(crate) struct HasOne {
    /// Target type
    pub(crate) ty: syn::Type,

    /// Field on target that the relation references
    pub(crate) pair: Option<Vec<syn::Ident>>,

    /// Field-name segments of a `#[has_one(via = a.b)]` multi-step relation.
    pub(crate) via: Option<Vec<syn::Ident>>,
}

impl HasOne {
    pub(super) fn from_ast(
        attr: &syn::Attribute,
        ty: &syn::Type,
        _span: proc_macro2::Span,
    ) -> syn::Result<Self> {
        let RelationAttrs { pair, via } = parse_has_relation_attrs(attr)?;

        Ok(Self {
            ty: ty.clone(),
            pair,
            via,
        })
    }
}

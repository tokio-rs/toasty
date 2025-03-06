use super::Name;

#[derive(Debug)]
pub(crate) struct HasMany {
    /// Target type
    pub(crate) ty: syn::Type,

    /// Singular field name
    pub(crate) singular: Name,
}

impl HasMany {
    pub(super) fn from_ast(name: &syn::Ident, ty: &syn::Type) -> syn::Result<HasMany> {
        let syn::Type::Slice(ty_slice) = ty else {
            return Err(syn::Error::new_spanned(ty, "expected slice type (`[_]`"));
        };

        let singular = Name::from_str(&std_util::str::singularize(&name.to_string()), name.span());

        Ok(HasMany {
            ty: (*ty_slice.elem).clone(),
            singular,
        })
    }
}

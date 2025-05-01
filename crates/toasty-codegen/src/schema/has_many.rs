use super::Name;

#[derive(Debug)]
pub(crate) struct HasMany {
    /// Target type
    pub(crate) ty: syn::Type,

    /// Singular field name
    pub(crate) singular: Name,

    /// Insert method ident
    pub(crate) insert_ident: syn::Ident,
}

impl HasMany {
    pub(super) fn from_ast(name: &syn::Ident, ty: &syn::Type) -> syn::Result<Self> {
        let syn::Type::Slice(ty_slice) = ty else {
            return Err(syn::Error::new_spanned(ty, "expected slice type (`[_]`"));
        };

        let singular = Name::from_str(&std_util::str::singularize(&name.to_string()), name.span());
        let insert_ident = syn::Ident::new(&format!("insert_{}", singular.ident), name.span());

        Ok(Self {
            ty: (*ty_slice.elem).clone(),
            singular,
            insert_ident,
        })
    }
}

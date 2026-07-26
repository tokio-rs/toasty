pub(crate) fn parse_comment(attr: &syn::Attribute) -> syn::Result<syn::LitStr> {
    let syn::Meta::NameValue(meta) = &attr.meta else {
        return Err(syn::Error::new_spanned(
            attr,
            "expected `comment = \"text\"`",
        ));
    };
    let syn::Expr::Lit(lit) = &meta.value else {
        return Err(syn::Error::new_spanned(
            attr,
            "expected `comment = \"text\"`",
        ));
    };
    let syn::Lit::Str(comment) = &lit.lit else {
        return Err(syn::Error::new_spanned(
            attr,
            "expected `comment = \"text\"`",
        ));
    };

    let value = comment.value();
    if value.trim().is_empty() {
        return Err(syn::Error::new_spanned(
            comment,
            "comment must not be empty",
        ));
    }
    if value.contains('\0') {
        return Err(syn::Error::new_spanned(
            comment,
            "comment must not contain a NUL byte",
        ));
    }

    Ok(comment.clone())
}

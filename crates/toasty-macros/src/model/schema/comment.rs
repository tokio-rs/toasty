pub(crate) fn validate_comment(comment: syn::LitStr) -> syn::Result<syn::LitStr> {
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

    Ok(comment)
}

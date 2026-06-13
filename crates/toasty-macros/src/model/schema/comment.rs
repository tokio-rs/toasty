pub(crate) fn parse_comment_attr(attr: &syn::Attribute) -> syn::Result<syn::LitStr> {
    if let syn::Meta::NameValue(meta) = &attr.meta {
        let syn::Expr::Lit(lit) = &meta.value else {
            return Err(expected(attr));
        };
        let syn::Lit::Str(lit) = &lit.lit else {
            return Err(expected(attr));
        };
        return Ok(lit.clone());
    }

    attr.parse_args().map_err(|_| expected(attr))
}

fn expected(attr: &syn::Attribute) -> syn::Error {
    syn::Error::new_spanned(
        attr,
        "expected `#[comment(\"text\")]` or `#[comment = \"text\"]`",
    )
}

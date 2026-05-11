use proc_macro2::TokenStream;

pub(crate) fn int(v: usize) -> TokenStream {
    use std::str::FromStr;
    TokenStream::from_str(&v.to_string()).expect("failed to parse int")
}

/// Creates a new identifier prefixed with `__toasty_` to avoid name collisions
/// with user-defined types in generated code (e.g., generic type parameters).
pub(crate) fn ident(name: &str) -> syn::Ident {
    quote::format_ident!("__toasty_{name}")
}

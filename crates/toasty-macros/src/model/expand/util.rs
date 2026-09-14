use proc_macro2::{Group, Span, TokenStream, TokenTree};

pub(crate) fn int(v: usize) -> TokenStream {
    use std::str::FromStr;
    TokenStream::from_str(&v.to_string()).expect("failed to parse int")
}

/// Rewrite every token's span to `span`, recursing into delimited groups.
///
/// `quote_spanned!` only spans the literal tokens it emits; interpolated
/// `#vars` keep their original spans. When the span of an *entire* emitted
/// fragment must point at one place — e.g. a synthesized type annotation whose
/// trait-bound failure should blame a specific source token rather than the
/// derive call site — run the fragment through this first.
pub(crate) fn respan(tokens: TokenStream, span: Span) -> TokenStream {
    tokens
        .into_iter()
        .map(|tree| match tree {
            TokenTree::Group(group) => {
                let mut respanned = Group::new(group.delimiter(), respan(group.stream(), span));
                respanned.set_span(span);
                TokenTree::Group(respanned)
            }
            mut tree => {
                tree.set_span(span);
                tree
            }
        })
        .collect()
}

/// Creates a new identifier prefixed with `__toasty_` to avoid name collisions
/// with user-defined types in generated code (e.g., generic type parameters).
pub(crate) fn ident(name: &str) -> syn::Ident {
    quote::format_ident!("__toasty_{name}")
}

/// Return the Rust method name an identifier occupies, without raw-ident syntax.
pub(crate) fn bare_ident_name(ident: &syn::Ident) -> String {
    let name = ident.to_string();
    name.strip_prefix("r#").unwrap_or(&name).to_string()
}

pub(crate) fn ident_is_reserved(ident: &syn::Ident, reserved: &[&str]) -> bool {
    let name = bare_ident_name(ident);
    reserved.contains(&name.as_str())
}

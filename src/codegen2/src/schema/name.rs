use proc_macro2::Span;
use std_util::str;

#[derive(Debug)]
pub(crate) struct Name {
    pub(crate) parts: Vec<String>,
    pub(crate) ident: syn::Ident,
    pub(crate) const_ident: syn::Ident,
}

impl Name {
    pub(crate) fn from_ident(ident: &syn::Ident) -> Name {
        Name::from_str(&ident.to_string(), ident.span())
    }

    pub(crate) fn from_str(src: &str, span: Span) -> Name {
        // TODO: improve logic
        let snake = str::snake_case(src);
        let parts: Vec<_> = snake.split("_").map(String::from).collect();

        let ident = syn::Ident::new(&parts.join("_"), span);
        let const_ident = syn::Ident::new(&parts.join("_").to_uppercase(), span);

        Name {
            parts,
            ident,
            const_ident,
        }
    }
}

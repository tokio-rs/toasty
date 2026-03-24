use heck::ToSnakeCase;
use proc_macro2::Span;

#[derive(Debug)]
pub(crate) struct Name {
    /// Name parts
    pub(crate) parts: Vec<String>,

    /// field/var identifier
    pub(crate) ident: syn::Ident,
}

impl Name {
    pub(crate) fn from_ident(ident: &syn::Ident) -> Self {
        Self::from_str(&ident.to_string(), ident.span())
    }

    pub(crate) fn from_str(src: &str, span: Span) -> Self {
        // TODO: improve logic
        let snake = src.to_snake_case();
        let parts: Vec<_> = snake.split("_").map(String::from).collect();

        let ident = syn::Ident::new(&parts.join("_"), span);

        Self { parts, ident }
    }
}

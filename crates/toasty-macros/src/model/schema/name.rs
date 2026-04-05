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
        // TODO: improve logic. There are a bunch of issues going on here. The
        // big one is, unnamed fields call this method passing in names like
        // `_0`. `to_snake_case` strips leading underscores (e.g. "_0" → "0"),
        // so we work aorund it by checking if the first character is a digit.
        // Lame, but it works (for now) without a bigger refactor. Preserve the
        let snake = src.to_snake_case();
        let snake = if snake.starts_with(|c: char| c.is_ascii_digit()) {
            src.to_string()
        } else {
            snake
        };
        let parts: Vec<_> = snake.split("_").map(String::from).collect();

        let ident = syn::Ident::new(&parts.join("_"), span);

        Self { parts, ident }
    }

    pub(crate) fn with_prefix(&self, prefix: &str) -> String {
        // Another hack (handling the same case as described in from_str).
        let name = self.ident.to_string();

        if name.starts_with("_") {
            format!("{prefix}{name}")
        } else {
            format!("{prefix}_{name}")
        }
    }
}

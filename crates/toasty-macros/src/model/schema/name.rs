use heck::ToSnakeCase;
use proc_macro2::Span;

#[derive(Debug)]
pub(crate) struct Name {
    /// Name parts
    pub(crate) parts: Vec<String>,

    /// Snake-case form of the name (`parts` joined by `_`), without any
    /// raw-identifier (`r#`) prefix.
    pub(crate) snake_case: String,

    /// field/var identifier
    pub(crate) ident: syn::Ident,
}

impl Name {
    pub(crate) fn from_ident(ident: &syn::Ident) -> Self {
        Self::from_str(&ident.to_string(), ident.span())
    }

    pub(crate) fn from_str(src: &str, span: Span) -> Self {
        // Strip the raw identifier prefix (`r#`) if present so it does not get
        // mangled by snake-case conversion (e.g. `r#type` → `r_type`).
        let (raw, src) = match src.strip_prefix("r#") {
            Some(stripped) => (true, stripped),
            None => (false, src),
        };

        let snake = src.to_snake_case();
        let parts: Vec<_> = snake.split("_").map(String::from).collect();

        let snake_case = parts.join("_");
        let ident = if raw {
            syn::Ident::new_raw(&snake_case, span)
        } else {
            syn::Ident::new(&snake_case, span)
        };

        Self {
            parts,
            snake_case,
            ident,
        }
    }

    /// The bare snake-case name, without any `r#` prefix.
    pub(crate) fn as_str(&self) -> &str {
        &self.snake_case
    }

    pub(crate) fn with_prefix(&self, prefix: &str) -> String {
        // Use the bare name (without any `r#` prefix) so the result is a valid
        // Rust identifier.
        format!("{prefix}_{}", self.snake_case)
    }
}

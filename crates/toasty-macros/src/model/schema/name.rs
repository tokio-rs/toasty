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
        // Rust identifier. Another hack: handles the `_0` case described in
        // `from_str` by checking for a leading underscore.
        let name = &self.snake_case;

        if name.starts_with("_") {
            format!("{prefix}{name}")
        } else {
            format!("{prefix}_{name}")
        }
    }
}

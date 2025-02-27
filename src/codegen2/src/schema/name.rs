use std_util::str;

#[derive(Debug)]
pub(crate) struct Name {
    pub(crate) parts: Vec<String>,
    pub(crate) ident: syn::Ident,
    pub(crate) const_ident: syn::Ident,
}

impl Name {
    pub(crate) fn from_ident(ident: &syn::Ident) -> Name {
        // TODO: improve logic
        let snake = str::snake_case(&ident.to_string());
        let parts: Vec<_> = snake.split("_").map(String::from).collect();

        let const_ident = syn::Ident::new(&parts.join("_").to_uppercase(), ident.span());

        Name {
            parts,
            ident: ident.clone(),
            const_ident,
        }
    }
}

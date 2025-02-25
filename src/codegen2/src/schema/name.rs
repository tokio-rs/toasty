use std_util::str;

#[derive(Debug)]
pub(crate) struct Name {
    pub(crate) parts: Vec<String>,
}

impl Name {
    pub(crate) fn from_ident(ident: &syn::Ident) -> Name {
        // TODO: improve logic
        let snake = str::snake_case(&ident.to_string());
        let parts = snake.split("_").map(String::from).collect();
        Name { parts }
    }
}

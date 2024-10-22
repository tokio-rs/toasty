use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Ident {
    name: String,
}

impl Ident {
    pub(crate) fn new(name: String) -> Ident {
        Ident { name }
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.name
    }

    pub(crate) fn to_string(&self) -> String {
        self.name.clone()
    }
}

impl Parse for Ident {
    fn parse(p: &mut Parser<'_>) -> Result<Self> {
        let token = p.next_token();

        match Ident::from_token(token.as_ref()) {
            Some(ident) => Ok(ident),
            token => todo!("expected Ident, got {:?}", token),
        }
    }
}

impl Peek for Ident {
    fn from_token(token: Option<&Token>) -> Option<Self> {
        match token {
            Some(Token::Ident(ident)) => Some(ident.clone()),
            _ => None,
        }
    }
}

impl AsRef<str> for Ident {
    fn as_ref(&self) -> &str {
        self.name.as_ref()
    }
}

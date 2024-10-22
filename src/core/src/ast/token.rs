use super::*;

#[derive(Debug, Clone)]
pub(crate) enum Token {
    Ident(Ident),
    Punct(Punct),
}

impl Parse for Token {
    fn parse(p: &mut Parser<'_>) -> Result<Self> {
        p.next_token().ok_or_else(|| todo!())
    }
}

impl Peek for Token {
    fn from_token(token: Option<&Token>) -> Option<Self> {
        token.cloned()
    }
}

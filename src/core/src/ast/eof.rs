use super::*;

#[derive(Debug)]
pub(crate) struct Eof;

impl Parse for Eof {
    fn parse(p: &mut Parser<'_>) -> Result<Self> {
        if p.is_eof() {
            Ok(Eof)
        } else {
            todo!("expected EOF, got {:?}", p.peek::<Token>());
        }
    }
}

impl Peek for Eof {
    fn from_token(token: Option<&Token>) -> Option<Self> {
        if token.is_none() {
            Some(Eof)
        } else {
            None
        }
    }
}

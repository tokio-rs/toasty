mod lexer;
use lexer::Lexer;

mod peek;
pub(crate) use peek::Peek;

mod parser;
pub(crate) use parser::Parser;

pub(crate) type Result<T> = std::result::Result<T, crate::Error>;

use super::*;

pub(crate) trait Parse: Sized {
    fn parse(parser: &mut Parser<'_>) -> Result<Self>;
}

pub(crate) fn from_str(src: &str) -> Result<Schema> {
    let mut parser = Parser::new(Lexer::new(src));
    Schema::parse(&mut parser)
}

impl<T: Peek> Parse for Option<T> {
    fn parse(p: &mut Parser<'_>) -> Result<Self> {
        if p.is_next::<T>() {
            Ok(Some(p.parse()?))
        } else {
            Ok(None)
        }
    }
}

impl<T: Parse, U: Parse> Parse for (T, U) {
    fn parse(p: &mut Parser<'_>) -> Result<Self> {
        Ok((p.parse()?, p.parse()?))
    }
}

impl<T: Parse> Parse for Box<T> {
    fn parse(p: &mut Parser<'_>) -> Result<Self> {
        Ok(Box::new(p.parse()?))
    }
}

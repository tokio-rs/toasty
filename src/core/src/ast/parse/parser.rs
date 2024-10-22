use super::*;

pub(crate) struct Parser<'a> {
    lexer: Lexer<'a>,
}

impl<'a> Parser<'a> {
    pub(crate) fn new(lexer: Lexer<'a>) -> Parser<'a> {
        Parser { lexer }
    }

    pub(crate) fn parse<T: Parse>(&mut self) -> Result<T> {
        T::parse(self)
    }

    pub(crate) fn parse_repeated_while<T: Parse, U: Peek>(&mut self) -> Result<Vec<T>> {
        let mut ret = vec![];

        while self.is_next::<U>() {
            ret.push(self.parse()?);
        }

        Ok(ret)
    }

    pub(crate) fn parse_repeated_until<T: Parse, U: Peek>(&mut self) -> Result<Vec<T>> {
        let mut ret = vec![];

        while !self.is_next::<U>() {
            ret.push(self.parse()?);
        }

        Ok(ret)
    }

    pub(crate) fn is_next<T: Peek>(&mut self) -> bool {
        self.is_nth::<T>(0)
    }

    pub(crate) fn is_nth<T: Peek>(&mut self, n: usize) -> bool {
        T::is_next(self.lexer.peek_nth(n))
    }

    pub(crate) fn peek<T: Peek>(&mut self) -> Option<T> {
        self.peek_nth(0)
    }

    pub(crate) fn peek_nth<T: Peek>(&mut self, n: usize) -> Option<T> {
        T::from_token(self.lexer.peek_nth(n))
    }

    pub(crate) fn next_token(&mut self) -> Option<Token> {
        self.lexer.next()
    }

    pub(crate) fn is_eof(&mut self) -> bool {
        self.peek::<Token>().is_none()
    }
}

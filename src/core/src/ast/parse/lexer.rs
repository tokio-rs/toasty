use crate::ast::*;

use std::collections::VecDeque;

pub(crate) struct Lexer<'a> {
    src: &'a str,
    next: VecDeque<Token>,
}

impl<'a> Lexer<'a> {
    pub(crate) fn new(src: &'a str) -> Lexer<'a> {
        Lexer {
            src,
            next: VecDeque::new(),
        }
    }

    pub(crate) fn next(&mut self) -> Option<Token> {
        self.lex_n(1);
        self.next.pop_front()
    }

    pub(crate) fn peek_nth(&mut self, n: usize) -> Option<&Token> {
        self.lex_n(n + 1);
        self.next.get(n)
    }

    fn lex_n(&mut self, n: usize) {
        if self.next.len() >= n {
            return;
        }

        // First, skip whitespace
        self.skip_whitespace();

        if let Some(ch) = self.try_next_char() {
            let token = match ch {
                // '@' => At.into(),
                ':' => Colon.into(),
                ';' => SemiColon.into(),
                ',' => Comma.into(),
                '(' => LParen.into(),
                ')' => RParen.into(),
                '{' => LBrace.into(),
                '}' => RBrace.into(),
                '[' => LBracket.into(),
                ']' => RBracket.into(),
                '=' => Eq.into(),
                '<' => Lt.into(),
                '>' => Gt.into(),
                '#' => Pound.into(),
                /*
                '-' => match self.peek_char()? {
                    Some('>') => {
                        self.consume(1);
                        token::RArrow.into()
                    }
                    Some(_) => todo!(""),
                    None => todo!("sigh"),
                },
                '"' => {
                    let mut s = String::new();

                    loop {
                        s.push(match self.next_char() {
                            '"' => break,
                            '\\' => todo!("handle escape sequences"),
                            ch => ch,
                        });
                    }

                    LitStr(s).into()
                }
                '\'' => {
                    todo!()
                }
                */
                ch if ch.is_alphabetic() => {
                    let mut ident = String::new();
                    ident.push(ch);

                    while let Some(ch) = self.take_if(ident_ch) {
                        ident.push(ch);
                    }

                    Token::Ident(Ident::new(ident))
                }
                ch => todo!("unexpected character {:?}", ch),
            };

            self.next.push_back(token);
        }
    }

    /*
    fn next_char(&mut self) -> char {
        match self.try_next_char() {
            Some(ch) => ch,
            None => todo!(),
        }
    }
    */

    fn try_next_char(&mut self) -> Option<char> {
        match self.src.chars().next() {
            Some(ch) => {
                self.consume(ch.len_utf8());
                Some(ch)
            }
            None => None,
        }
    }

    fn peek_char(&mut self) -> Option<char> {
        self.peek_char_n(0)
    }

    fn peek_char_n(&mut self, n: usize) -> Option<char> {
        self.src.chars().nth(n)
    }

    fn take_if<P>(&mut self, predicate: P) -> Option<char>
    where
        P: FnOnce(char) -> bool,
    {
        match self.peek_char() {
            Some(ch) if predicate(ch) => {
                self.consume(ch.len_utf8());
                Some(ch)
            }
            _ => None,
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek_char() {
            match ch {
                '/' => {
                    if self.peek_char_n(1) == Some('*') {
                        self.consume(2);
                        self.skip_block_comment();
                    } else if self.peek_char_n(1) == Some('/') {
                        self.skip_line_comment();
                    } else {
                        return;
                    }
                }
                ch if ch.is_whitespace() => {
                    self.consume(ch.len_utf8());
                }
                _ => return,
            }
        }
    }

    fn skip_block_comment(&mut self) {
        while let Some(ch) = self.peek_char() {
            match ch {
                '*' => {
                    self.consume(1);

                    if self.peek_char() == Some('/') {
                        self.consume(1);
                        return;
                    }
                }
                _ => {
                    self.consume(ch.len_utf8());
                }
            }
        }
    }

    fn skip_line_comment(&mut self) {
        while let Some(ch) = self.peek_char() {
            match ch {
                '\n' => {
                    self.consume(1);
                    return;
                }
                _ => self.consume(ch.len_utf8()),
            }
        }
    }

    fn consume(&mut self, amount: usize) {
        let (_, src) = self.src.split_at(amount);
        self.src = src;
    }
}

fn ident_ch(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric()
}

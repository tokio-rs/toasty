use super::*;

macro_rules! punct {
    ( $( $i:ident ;)* ) => {
        $(
            #[derive(Debug, Clone, PartialEq)]
            pub(crate) struct $i;

            impl Parse for $i {
                fn parse(p: &mut Parser<'_>) -> Result<$i> {
                    let token = p.next_token();
                    match Self::from_token(token.as_ref()) {
                        Some(punct) => Ok(punct),
                        _ => todo!("expected `{}`, got {:?}", stringify!($t), token),
                    }
                }
            }

            impl Peek for $i {
                fn from_token(token: Option<&Token>) -> Option<Self> {
                    match token {
                        Some(Token::Punct(Punct::$i(punct))) => Some(punct.clone()),
                        _ => None,
                    }
                }
            }

            impl From<$i> for Token {
                fn from(src: $i) -> Token {
                    Token::Punct(Punct::$i(src))
                }
            }
        )*

        #[derive(Debug, Clone)]
        pub(crate) enum Punct {
            $(
                $i($i),
            )*
        }
    }
}

punct! {
    // At;
    Colon;
    SemiColon;
    LBrace;
    RBrace;
    LBracket;
    RBracket;
    Comma;
    Eq;
    Lt;
    Gt;
    Pound;
    LParen;
    RParen;
    /*
    LBrace;
    RBrace;
    RArrow;
    */
    PathSep;
}

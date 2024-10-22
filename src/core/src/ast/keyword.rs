use super::*;

macro_rules! define_keyword {
    ( $( $l:literal => $i:ident; )* ) => {
        $(
            #[derive(Debug)]
            pub(crate) struct $i;

            impl Parse for $i {
                fn parse(parser: &mut Parser<'_>) -> Result<$i> {
                    match parser.parse::<Token>()? {
                        Token::Ident(ident) => {
                            if ident.as_str() == $l {
                                Ok($i)
                            } else {
                                todo!()
                            }
                        }
                        token => todo!("unexpected {:#?}", token),
                    }
                }
            }

            impl Peek for $i {
                fn from_token(token: Option<&Token>) -> Option<Self> {
                    match token {
                        Some(Token::Ident(ident)) if ident.as_str() == $l => Some(Self),
                        _ => None
                    }
                }
            }
        )*
    };
}

define_keyword! {
    "model" => Model;
    "table" => Table;

    // The Option keyword for `Option<_>` types
    "Option" => Opt;
}

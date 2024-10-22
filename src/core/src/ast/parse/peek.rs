use super::*;

pub(crate) trait Peek: Parse {
    fn from_token(token: Option<&Token>) -> Option<Self>;

    fn is_next(token: Option<&Token>) -> bool {
        Self::from_token(token).is_some()
    }
}

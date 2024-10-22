use super::*;

#[derive(Debug, Clone, PartialEq)]
pub enum ExprPattern<'stmt> {
    /// Tests if a string expression starts with a particular substring.
    BeginsWith(ExprBeginsWith<'stmt>),

    Like(ExprLike<'stmt>),
}

use super::*;

#[derive(Debug, Clone)]
pub enum ExprPattern {
    /// Tests if a string expression starts with a particular substring.
    BeginsWith(ExprBeginsWith),

    Like(ExprLike),
}

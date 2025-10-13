use super::{ExprBeginsWith, ExprLike};

#[derive(Debug, Clone, PartialEq)]
pub enum ExprPattern {
    /// Tests if a string expression starts with a particular substring.
    BeginsWith(ExprBeginsWith),

    Like(ExprLike),
}

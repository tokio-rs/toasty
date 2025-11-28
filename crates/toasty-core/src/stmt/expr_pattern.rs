use super::{ExprBeginsWith, ExprLike};

/// A pattern matching expression.
///
/// Groups string pattern matching operations like prefix matching and SQL LIKE.
#[derive(Debug, Clone, PartialEq)]
pub enum ExprPattern {
    /// Tests if a string starts with a prefix.
    BeginsWith(ExprBeginsWith),

    /// Tests if a string matches a SQL "like" pattern.
    Like(ExprLike),
}

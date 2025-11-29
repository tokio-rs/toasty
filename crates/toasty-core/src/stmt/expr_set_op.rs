use super::{ExprSet, SetOp};

/// A set operation combining multiple queries.
///
/// Applies a set operator (union, except, intersect) to combine the results of
/// multiple queries into a single result set.
///
/// # Examples
///
/// ```text
/// SELECT ... UNION SELECT ...       // combines with union
/// SELECT ... EXCEPT SELECT ...      // removes matching rows
/// SELECT ... INTERSECT SELECT ...   // keeps only common rows
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ExprSetOp {
    /// The set operation to apply.
    pub op: SetOp,

    /// The queries to combine.
    pub operands: Vec<ExprSet>,
}

impl ExprSetOp {
    pub fn is_union(&self) -> bool {
        matches!(self.op, SetOp::Union)
    }
}

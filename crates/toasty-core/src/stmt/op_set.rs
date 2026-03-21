use std::fmt;

/// A SQL set operation that combines result sets from multiple queries.
///
/// Used by [`ExprSetOp`](super::ExprSetOp) to specify how multiple query
/// results are combined.
///
/// # Examples
///
/// ```
/// use toasty_core::stmt::SetOp;
///
/// let op = SetOp::Union;
/// assert_eq!(op.to_string(), "UNION");
/// ```
#[derive(Copy, Clone, PartialEq)]
pub enum SetOp {
    /// Combines results from multiple queries, including duplicates.
    Union,
    /// Returns rows from the first query that are not in the second.
    Except,
    /// Returns only rows common to both queries.
    Intersect,
}

impl SetOp {}

impl fmt::Display for SetOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SetOp::Union => "UNION".fmt(f),
            SetOp::Except => "EXCEPT".fmt(f),
            SetOp::Intersect => "INTERSECT".fmt(f),
        }
    }
}

impl fmt::Debug for SetOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

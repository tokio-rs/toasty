use super::{Expr, SourceTableId};

/// A join clause within a [`TableWithJoins`](super::TableWithJoins).
///
/// References a table by [`SourceTableId`] and specifies the join type and
/// condition via a [`JoinOp`].
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{Join, JoinOp, SourceTableId, Expr};
///
/// let join = Join {
///     table: SourceTableId(1),
///     constraint: JoinOp::Left(Expr::TRUE),
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Join {
    /// Index of the table to join, referencing [`SourceTable::tables`](super::SourceTable).
    pub table: SourceTableId,

    /// The join type and condition.
    pub constraint: JoinOp,
}

/// The type of join and its ON condition.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::{JoinOp, Expr};
///
/// let op = JoinOp::Left(Expr::TRUE);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum JoinOp {
    /// An `INNER JOIN` with the given ON condition.
    Inner(Expr),

    /// A `LEFT JOIN` with the given ON condition.
    Left(Expr),
}

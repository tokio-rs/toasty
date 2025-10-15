use super::{Expr, SourceTableId};

#[derive(Debug, Clone, PartialEq)]
pub struct Join {
    /// The table to join
    pub table: SourceTableId,

    /// The join condition
    pub constraint: JoinOp,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JoinOp {
    Left(Expr),
}

use super::{Expr, SourceTableId};

#[derive(Debug, Clone)]
pub struct Join {
    /// The table to join
    pub table: SourceTableId,

    /// The join condition
    pub constraint: JoinOp,
}

#[derive(Debug, Clone)]
pub enum JoinOp {
    Left(Expr),
}

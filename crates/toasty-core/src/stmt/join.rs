use super::{Expr, TableRef};

#[derive(Debug, Clone, PartialEq)]
pub struct Join {
    /// The table to join
    pub table: TableRef,

    /// The join condition
    pub constraint: JoinOp,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JoinOp {
    Left(Expr),
}

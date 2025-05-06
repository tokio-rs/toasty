use super::{Expr, TableRef};

#[derive(Debug, Clone)]
pub struct Join {
    /// The table to join
    pub table: TableRef,

    /// The join condition
    pub constraint: JoinOp,
}

#[derive(Debug, Clone)]
pub enum JoinOp {
    Left(Expr),
}

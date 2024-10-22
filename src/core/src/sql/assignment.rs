use super::*;

#[derive(Debug, Clone)]
pub struct Assignment<'stmt> {
    /// For now, only assign to a column
    pub target: ColumnId,

    /// Value to assign to the column
    pub value: Expr<'stmt>,
}

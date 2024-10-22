use super::*;

#[derive(Debug, Clone)]
pub struct Update<'stmt> {
    /// TABLE to update
    pub table: TableWithJoins,

    /// Column assignments
    pub assignments: Vec<Assignment<'stmt>>,

    /// WHERE clause
    pub selection: Option<Expr<'stmt>>,

    /// Not part of SQL. An optional pre-condition before applying the update to
    /// the row. The difference between a pre-condition and a where clause is
    /// that the caller can tell that a row did match the where clause but
    /// failed the precondition. For example, this is useful when the caller
    /// needs to tell the difference.
    pub pre_condition: Option<Expr<'stmt>>,

    /// RETURNING clause
    pub returning: Option<Vec<Expr<'stmt>>>,
}

impl<'stmt> From<Update<'stmt>> for Statement<'stmt> {
    fn from(value: Update<'stmt>) -> Self {
        Statement::Update(value)
    }
}

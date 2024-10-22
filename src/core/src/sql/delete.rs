use super::*;

#[derive(Debug, Clone)]
pub struct Delete<'stmt> {
    /// `FROM` table, can include joins
    pub from: Vec<TableWithJoins>,

    /// `WHERE`
    pub selection: Option<Expr<'stmt>>,

    /// What to return, if anything
    pub returning: Option<Vec<Expr<'stmt>>>,
}

impl<'stmt> From<Delete<'stmt>> for Statement<'stmt> {
    fn from(value: Delete<'stmt>) -> Self {
        Statement::Delete(value)
    }
}

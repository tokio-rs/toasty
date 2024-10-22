use super::*;

#[derive(Debug, Clone)]
pub struct Insert<'stmt> {
    /// The table to insert into
    pub table: TableId,

    /// Columns to insert into
    pub columns: Vec<ColumnId>,

    /// A SQL query (or values) used to insert
    pub source: Box<Query<'stmt>>,

    /// If the insert statement returns something, then this field is set.
    pub returning: Option<Vec<Expr<'stmt>>>,
}

impl<'stmt> From<Insert<'stmt>> for Statement<'stmt> {
    fn from(value: Insert<'stmt>) -> Self {
        Statement::Insert(value)
    }
}

use super::Expr;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ExprColumn {
    /// Which query the alias is listed in
    pub nesting: usize,

    /// The index of the alias in the `FROM` (or equivalent) clause
    pub table: usize,

    /// The index of the column in the table
    pub column: usize,
}

impl Expr {
    pub fn column(column: impl Into<ExprColumn>) -> Self {
        column.into().into()
    }

    pub fn is_column(&self) -> bool {
        matches!(self, Self::Column(_))
    }
}

impl ExprColumn {
    pub fn new(nesting: usize, table: usize, column: usize) -> Self {
        Self { nesting, table, column }
    }
}

impl From<ExprColumn> for Expr {
    fn from(value: ExprColumn) -> Self {
        Self::Column(value)
    }
}


use super::*;

#[derive(Debug, Clone, PartialEq)]
pub enum ExprColumn {
    /// Directly reference a column
    Column(ColumnId),

    /// Reference a column aliased in `FROM` or equivalent clause
    Alias {
        /// Which query the alias is listed in
        nesting: usize,

        /// The index of the alias in the `FROM` (or equivalent) clause
        table: usize,

        /// The index of the column in the table
        column: usize,
    },
}

impl Expr {
    pub fn column(column: impl Into<ExprColumn>) -> Expr {
        column.into().into()
    }

    pub fn is_column(&self) -> bool {
        matches!(self, Expr::Column(_))
    }
}

impl ExprColumn {
    pub fn references(&self, column_id: ColumnId) -> bool {
        match self {
            ExprColumn::Column(id) => id == &column_id,
            ExprColumn::Alias { .. } => todo!(),
        }
    }

    pub fn try_to_column_id(&self) -> Option<ColumnId> {
        match self {
            ExprColumn::Column(id) => Some(*id),
            ExprColumn::Alias { .. } => None,
        }
    }
}

impl From<ExprColumn> for Expr {
    fn from(value: ExprColumn) -> Self {
        Expr::Column(value)
    }
}

impl From<&Column> for ExprColumn {
    fn from(value: &Column) -> Self {
        value.id.into()
    }
}

impl From<&Column> for Expr {
    fn from(value: &Column) -> Self {
        value.id.into()
    }
}

impl From<ColumnId> for ExprColumn {
    fn from(value: ColumnId) -> Self {
        ExprColumn::Column(value)
    }
}

impl From<ColumnId> for Expr {
    fn from(value: ColumnId) -> Self {
        Expr::column(value)
    }
}

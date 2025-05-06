use super::*;

#[derive(Debug, Clone, Eq, PartialEq)]
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
    pub fn column(column: impl Into<ExprColumn>) -> Self {
        column.into().into()
    }

    pub fn is_column(&self) -> bool {
        matches!(self, Self::Column(_))
    }
}

impl ExprColumn {
    pub fn references(&self, column_id: ColumnId) -> bool {
        match self {
            Self::Column(id) => id == &column_id,
            Self::Alias { .. } => todo!(),
        }
    }

    pub fn try_to_column_id(&self) -> Option<ColumnId> {
        match self {
            Self::Column(id) => Some(*id),
            Self::Alias { .. } => None,
        }
    }
}

impl From<ExprColumn> for Expr {
    fn from(value: ExprColumn) -> Self {
        Self::Column(value)
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
        Self::Column(value)
    }
}

impl From<ColumnId> for Expr {
    fn from(value: ColumnId) -> Self {
        Self::column(value)
    }
}

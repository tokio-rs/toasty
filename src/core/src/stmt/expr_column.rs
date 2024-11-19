use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprColumn {
    pub column: ColumnId,
}

impl Expr {
    pub fn column(column: impl Into<ColumnId>) -> Expr {
        ExprColumn {
            column: column.into(),
        }
        .into()
    }
}

impl From<ExprColumn> for Expr {
    fn from(value: ExprColumn) -> Self {
        Expr::Column(value)
    }
}

impl From<&Column> for ExprColumn {
    fn from(value: &Column) -> Self {
        ExprColumn { column: value.id }
    }
}

impl From<&Column> for Expr {
    fn from(value: &Column) -> Self {
        Expr::column(value.id)
    }
}

impl From<ColumnId> for ExprColumn {
    fn from(value: ColumnId) -> Self {
        ExprColumn { column: value }
    }
}

impl From<ColumnId> for Expr {
    fn from(value: ColumnId) -> Self {
        Expr::column(value)
    }
}

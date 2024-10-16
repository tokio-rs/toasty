use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprColumn {
    pub column: ColumnId,
}

impl<'stmt> Expr<'stmt> {
    pub fn column(column: impl Into<ColumnId>) -> Expr<'stmt> {
        ExprColumn {
            column: column.into(),
        }
        .into()
    }
}

impl<'stmt> From<ExprColumn> for Expr<'stmt> {
    fn from(value: ExprColumn) -> Self {
        Expr::Column(value)
    }
}

impl<'stmt> From<&Column> for ExprColumn {
    fn from(value: &Column) -> Self {
        ExprColumn { column: value.id }
    }
}

impl<'stmt> From<&Column> for Expr<'stmt> {
    fn from(value: &Column) -> Self {
        Expr::column(value.id)
    }
}

impl<'stmt> From<ColumnId> for ExprColumn {
    fn from(value: ColumnId) -> Self {
        ExprColumn { column: value }
    }
}

impl<'stmt> From<ColumnId> for Expr<'stmt> {
    fn from(value: ColumnId) -> Self {
        Expr::column(value)
    }
}

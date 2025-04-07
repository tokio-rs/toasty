use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprColumn {
    /// Table being referenced
    pub table: TableRef,

    /// Index of column being referenced
    pub index: usize,
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
        self.table.references(column_id.table) && self.index == column_id.index
    }

    pub fn try_to_column_id(&self) -> Option<ColumnId> {
        if let TableRef::Table(table_id) = self.table {
            Some(ColumnId {
                table: table_id,
                index: self.index,
            })
        } else {
            None
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
        ExprColumn {
            table: value.table.into(),
            index: value.index,
        }
    }
}

impl From<ColumnId> for Expr {
    fn from(value: ColumnId) -> Self {
        Expr::column(value)
    }
}

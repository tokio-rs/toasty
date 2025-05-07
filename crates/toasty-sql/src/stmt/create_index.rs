use super::*;

use toasty_core::{
    schema::db::{Index, IndexOp, TableId},
    stmt,
};

#[derive(Debug, Clone)]
pub struct CreateIndex {
    /// Name of the index
    pub name: Name,

    /// Which table to index
    pub on: TableId,

    /// The columns to index
    pub columns: Vec<stmt::OrderByExpr>,

    /// When true, the index is unique
    pub unique: bool,
}

impl Statement {
    pub fn create_index(index: &Index) -> Self {
        CreateIndex {
            name: Name::from(&index.name[..]),
            on: index.on,
            columns: index
                .columns
                .iter()
                .map(|index_column| stmt::OrderByExpr {
                    expr: stmt::Expr::column(index_column.column),
                    order: match index_column.op {
                        IndexOp::Eq => None,
                        IndexOp::Sort(direction) => Some(direction),
                    },
                })
                .collect(),
            unique: index.unique,
        }
        .into()
    }
}

impl From<CreateIndex> for Statement {
    fn from(value: CreateIndex) -> Self {
        Self::CreateIndex(value)
    }
}

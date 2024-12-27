use super::*;

use crate::schema::{self, Index, TableId};

#[derive(Debug, Clone)]
pub struct CreateIndex {
    /// Name of the index
    pub name: Name,

    /// Which table to index
    pub on: TableId,

    /// The columns to index
    pub columns: Vec<ExprOrderBy>,

    /// When true, the index is unique
    pub unique: bool,
}

impl Statement {
    pub fn create_index(index: &Index) -> Statement {
        CreateIndex {
            name: Name::from(&index.name[..]),
            on: index.on,
            columns: index
                .columns
                .iter()
                .map(|index_column| ExprOrderBy {
                    expr: Expr::column(index_column.column),
                    order: match index_column.op {
                        schema::IndexOp::Eq => None,
                        schema::IndexOp::Sort(direction) => Some(direction),
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
        Statement::CreateIndex(value)
    }
}

use super::{Name, Statement};

use toasty_core::{
    schema::db::{ColumnId, Index, IndexOp, TableId},
    stmt,
};

#[derive(Debug, Clone)]
pub struct CreateIndex {
    /// Name of the index
    pub name: Name,

    /// Which table to index
    pub on: TableId,

    /// The columns to index (stored as ColumnIds for DDL)
    pub columns: Vec<(ColumnId, Option<stmt::Direction>)>,

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
                .map(|index_column| {
                    let direction = match index_column.op {
                        IndexOp::Eq => None,
                        IndexOp::Sort(direction) => Some(direction),
                    };
                    (index_column.column, direction)
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

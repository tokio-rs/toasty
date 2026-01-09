use std::collections::HashMap;

use crate::schema::{
    app::IndexId,
    db::{ColumnId, TableId},
};

use super::Schema;

#[derive(Default)]
pub struct RenameHints {
    tables: HashMap<TableId, TableId>,
    columns: HashMap<ColumnId, ColumnId>,
    indices: HashMap<IndexId, IndexId>,
}

impl RenameHints {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_table_hint(&mut self, from: TableId, to: TableId) {
        self.tables.insert(from, to);
    }

    pub fn add_column_hint(&mut self, from: ColumnId, to: ColumnId) {
        self.columns.insert(from, to);
    }

    pub fn add_index_hint(&mut self, from: IndexId, to: IndexId) {
        self.indices.insert(from, to);
    }

    pub fn get_table(&self, from: TableId) -> Option<TableId> {
        self.tables.get(&from).copied()
    }

    pub fn get_column(&self, from: ColumnId) -> Option<ColumnId> {
        self.columns.get(&from).copied()
    }

    pub fn get_index(&self, from: IndexId) -> Option<IndexId> {
        self.indices.get(&from).copied()
    }
}

pub struct DiffContext<'a> {
    from: &'a Schema,
    to: &'a Schema,

    rename_hints: &'a RenameHints,
}

impl<'a> DiffContext<'a> {
    pub fn new(from: &'a Schema, to: &'a Schema, rename_hints: &'a RenameHints) -> Self {
        Self {
            from,
            to,
            rename_hints,
        }
    }

    pub fn rename_hints(&self) -> &'a RenameHints {
        self.rename_hints
    }

    pub fn schema_from(&self) -> &'a Schema {
        self.from
    }

    pub fn schema_to(&self) -> &'a Schema {
        self.to
    }
}

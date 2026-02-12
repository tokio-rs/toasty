use super::{
    Column, ColumnId, DiffContext, Index, IndexId, RenameHints, Table, TableId, TablesDiff,
};

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Schema {
    pub tables: Vec<Table>,
}

impl Schema {
    pub fn column(&self, id: impl Into<ColumnId>) -> &Column {
        let id = id.into();
        self.table(id.table)
            .columns
            .get(id.index)
            .expect("invalid column ID")
    }

    pub fn column_mut(&mut self, id: impl Into<ColumnId>) -> &mut Column {
        let id = id.into();
        self.table_mut(id.table)
            .columns
            .get_mut(id.index)
            .expect("invalid column ID")
    }

    // NOTE: this is unlikely to confuse users given the context.
    #[allow(clippy::should_implement_trait)]
    pub fn index(&self, id: IndexId) -> &Index {
        self.table(id.table)
            .indices
            .get(id.index)
            .expect("invalid index ID")
    }

    // NOTE: this is unlikely to confuse users given the context.
    #[allow(clippy::should_implement_trait)]
    pub fn index_mut(&mut self, id: IndexId) -> &mut Index {
        self.table_mut(id.table)
            .indices
            .get_mut(id.index)
            .expect("invalid index ID")
    }

    pub fn table(&self, id: impl Into<TableId>) -> &Table {
        self.tables.get(id.into().0).expect("invalid table ID")
    }

    pub fn table_mut(&mut self, id: impl Into<TableId>) -> &mut Table {
        self.tables.get_mut(id.into().0).expect("invalid table ID")
    }
}

pub struct SchemaDiff<'a> {
    previous: &'a Schema,
    next: &'a Schema,
    tables: TablesDiff<'a>,
}

impl<'a> SchemaDiff<'a> {
    pub fn from(from: &'a Schema, to: &'a Schema, rename_hints: &'a RenameHints) -> Self {
        let cx = &DiffContext::new(from, to, rename_hints);
        Self {
            previous: from,
            next: to,
            tables: TablesDiff::from(cx, &from.tables, &to.tables),
        }
    }

    pub fn tables(&self) -> &TablesDiff<'a> {
        &self.tables
    }

    pub fn is_empty(&self) -> bool {
        self.tables.is_empty()
    }

    pub fn previous(&self) -> &'a Schema {
        self.previous
    }

    pub fn next(&self) -> &'a Schema {
        self.next
    }
}

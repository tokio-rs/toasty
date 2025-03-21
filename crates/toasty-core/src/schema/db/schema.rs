use super::*;

#[derive(Debug, Default)]
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

    // NOTE: this is unlikely to confuse users given the context.
    #[allow(clippy::should_implement_trait)]
    pub fn index(&self, id: IndexId) -> &Index {
        self.table(id.table)
            .indices
            .get(id.index)
            .expect("invalid index ID")
    }

    pub fn table(&self, id: impl Into<TableId>) -> &Table {
        self.tables.get(id.into().0).expect("invalid table ID")
    }
}

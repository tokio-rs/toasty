use super::TableId;

#[derive(Debug, Clone, PartialEq)]
pub enum TableRef {
    /// An aliased table (in a `FROM` statement or equivalent).
    Cte {
        /// What level of nesting the reference is compared to the CTE being
        /// referenced.
        nesting: usize,

        /// The index of the CTE in the `WITH` clause
        index: usize,
    },

    /// A defined table from the schema
    Table(TableId),
}

impl TableRef {
    pub fn references(&self, table_id: TableId) -> bool {
        match self {
            TableRef::Cte { .. } => false,
            TableRef::Table(id) => id == &table_id,
        }
    }
}

impl From<TableId> for TableRef {
    fn from(value: TableId) -> Self {
        TableRef::Table(value)
    }
}

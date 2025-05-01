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
            Self::Cte { .. } => false,
            Self::Table(id) => id == &table_id,
        }
    }

    pub fn is_cte(&self) -> bool {
        matches!(self, Self::Cte { .. })
    }
}

impl From<TableId> for TableRef {
    fn from(value: TableId) -> Self {
        Self::Table(value)
    }
}

use crate::stmt::{ExprArg, TableDerived};

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

    /// A table derived from a query
    Derived(TableDerived),

    /// A defined table from the schema
    Table(TableId),

    /// The table ref will be provided at a later time (and will become a
    /// derived table)
    Arg(ExprArg),
}

impl TableRef {
    pub fn references(&self, table_id: TableId) -> bool {
        match self {
            Self::Cte { .. } => false,
            Self::Derived { .. } => false,
            Self::Table(id) => id == &table_id,
            Self::Arg { .. } => todo!(),
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

impl From<ExprArg> for TableRef {
    fn from(value: ExprArg) -> Self {
        TableRef::Arg(value)
    }
}

impl PartialEq<TableId> for TableRef {
    fn eq(&self, other: &TableId) -> bool {
        self.references(*other)
    }
}

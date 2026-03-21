use crate::stmt::{ExprArg, TableDerived};

use super::TableId;

/// A reference to a table within a [`SourceTable`](super::SourceTable).
///
/// Each entry in [`SourceTable::tables`](super::SourceTable) is a `TableRef`
/// that identifies where data comes from: a schema table, a CTE, a derived
/// subquery, or a placeholder argument.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::stmt::TableRef;
/// use toasty_core::schema::db::TableId;
///
/// let table_ref = TableRef::Table(TableId(0));
/// assert!(table_ref.references(TableId(0)));
/// assert!(!table_ref.is_cte());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum TableRef {
    /// A reference to a CTE defined in a `WITH` clause.
    Cte {
        /// How many nesting levels up the CTE is defined relative to this
        /// reference.
        nesting: usize,

        /// The index of the CTE within the [`With::ctes`](super::With) vector.
        index: usize,
    },

    /// A derived table (inline subquery).
    Derived(TableDerived),

    /// A schema-defined table.
    Table(TableId),

    /// A placeholder that will be replaced with a derived table at a later
    /// compilation stage.
    Arg(ExprArg),
}

impl TableRef {
    /// Returns `true` if this ref points to the given schema table.
    pub fn references(&self, table_id: TableId) -> bool {
        match self {
            Self::Cte { .. } => false,
            Self::Derived { .. } => false,
            Self::Table(id) => id == &table_id,
            Self::Arg { .. } => todo!(),
        }
    }

    /// Returns `true` if this is a `Cte` reference.
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

use super::{Column, ColumnId, Index, IndexId, PrimaryKey};
use crate::stmt;

use std::fmt;

/// A database table with its columns, primary key, and indices.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::schema::db::{Table, TableId};
///
/// let table = Table::new(TableId(0), "users".to_string());
/// assert_eq!(table.name, "users");
/// assert!(table.columns.is_empty());
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Table {
    /// Uniquely identifies a table within the schema.
    pub id: TableId,

    /// Name of the table as it appears in the database.
    pub name: String,

    /// The table's columns, in order.
    pub columns: Vec<Column>,

    /// The table's primary key definition.
    pub primary_key: PrimaryKey,

    /// Secondary indices on this table.
    pub indices: Vec<Index>,
}

/// Uniquely identifies a table within a [`Schema`](super::Schema).
///
/// The inner `usize` is a zero-based index into [`Schema::tables`](super::Schema::tables).
///
/// # Examples
///
/// ```ignore
/// use toasty_core::schema::db::TableId;
///
/// let id = TableId(0);
/// assert_eq!(id.0, 0);
/// ```
#[derive(PartialEq, Eq, Clone, Copy, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableId(pub usize);

impl Table {
    /// Returns the `i`-th column of this table's primary key.
    ///
    /// # Panics
    ///
    /// Panics if `i` is out of bounds for the primary key column list.
    pub fn primary_key_column(&self, i: usize) -> &Column {
        &self.columns[self.primary_key.columns[i].index]
    }

    /// Returns an iterator over the columns that make up this table's primary key.
    pub fn primary_key_columns(&self) -> impl ExactSizeIterator<Item = &Column> + '_ {
        self.primary_key
            .columns
            .iter()
            .map(|column_id| &self.columns[column_id.index])
    }

    /// Returns the column identified by `id`.
    ///
    /// Only the column's `index` field is used; the `table` component is ignored.
    ///
    /// # Panics
    ///
    /// Panics if the column index is out of bounds.
    pub fn column(&self, id: impl Into<ColumnId>) -> &Column {
        &self.columns[id.into().index]
    }

    /// Resolves a single-step [`Projection`](stmt::Projection) to a column.
    ///
    /// # Panics
    ///
    /// Panics if the projection is empty or contains more than one step.
    pub fn resolve(&self, projection: &stmt::Projection) -> &Column {
        let [first, rest @ ..] = projection.as_slice() else {
            panic!("need at most one path step")
        };
        assert!(rest.is_empty());

        &self.columns[*first]
    }

    pub(crate) fn new(id: TableId, name: String) -> Self {
        Self {
            id,
            name,
            columns: vec![],
            primary_key: PrimaryKey {
                columns: vec![],
                index: IndexId {
                    table: id,
                    index: 0,
                },
            },
            indices: vec![],
        }
    }
}

impl TableId {
    pub(crate) fn placeholder() -> Self {
        Self(usize::MAX)
    }
}

impl fmt::Debug for TableId {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "TableId({})", self.0)
    }
}

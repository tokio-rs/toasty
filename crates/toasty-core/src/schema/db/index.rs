use super::{Column, ColumnId, Schema, TableId};
use crate::stmt;

use std::fmt;

/// A database index over one or more columns of a table.
///
/// Indices can be unique or non-unique, and can cover the primary key.
/// Each indexed column specifies an [`IndexOp`] (equality or sort) and an
/// [`IndexScope`] (partition or local).
///
/// # Examples
///
/// ```ignore
/// use toasty_core::schema::db::{Index, IndexColumn, IndexId, IndexOp, IndexScope, ColumnId, TableId};
///
/// let index = Index {
///     id: IndexId { table: TableId(0), index: 0 },
///     name: "idx_users_email".to_string(),
///     on: TableId(0),
///     columns: vec![IndexColumn {
///         column: ColumnId { table: TableId(0), index: 1 },
///         op: IndexOp::Eq,
///         scope: IndexScope::Local,
///     }],
///     unique: true,
///     primary_key: false,
/// };
///
/// assert!(index.unique);
/// assert_eq!(index.columns.len(), 1);
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Index {
    /// Uniquely identifies the index within the schema
    pub id: IndexId,

    /// Index name is unique within the schema
    pub name: String,

    /// The table being indexed
    pub on: TableId,

    /// Fields included in the index.
    pub columns: Vec<IndexColumn>,

    /// When `true`, indexed entries are unique
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "is_false"))]
    pub unique: bool,

    /// When `true`, the index indexes the model's primary key fields.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "is_false"))]
    pub primary_key: bool,
}

#[cfg(feature = "serde")]
fn is_false(b: &bool) -> bool {
    !*b
}

/// Uniquely identifies an index within a schema.
///
/// Combines the [`TableId`] of the owning table with the index's positional
/// offset in that table's index list.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::schema::db::{IndexId, TableId};
///
/// let id = IndexId { table: TableId(0), index: 1 };
/// assert_eq!(id.index, 1);
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IndexId {
    /// The table this index belongs to.
    pub table: TableId,
    /// Zero-based position of this index in the table's index list.
    pub index: usize,
}

/// A single column entry within an [`Index`].
///
/// Specifies which column is indexed, the comparison operation, and the scope
/// (partition vs. local).
///
/// # Examples
///
/// ```ignore
/// use toasty_core::schema::db::{IndexColumn, IndexOp, IndexScope, ColumnId, TableId};
///
/// let ic = IndexColumn {
///     column: ColumnId { table: TableId(0), index: 0 },
///     op: IndexOp::Eq,
///     scope: IndexScope::Local,
/// };
///
/// assert!(ic.scope.is_local());
/// ```
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IndexColumn {
    /// The column being indexed
    pub column: ColumnId,

    /// The comparison operation used to index the column
    pub op: IndexOp,

    /// Scope of the index
    pub scope: IndexScope,
}

/// The comparison operation used by an index column.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::schema::db::IndexOp;
/// use toasty_core::stmt::Direction;
///
/// let op = IndexOp::Sort(Direction::Asc);
/// assert!(matches!(op, IndexOp::Sort(_)));
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum IndexOp {
    /// Equality lookup.
    Eq,
    /// Sorted scan in the given direction.
    Sort(stmt::Direction),
}

/// Scope of an index column, relevant for distributed databases.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::schema::db::IndexScope;
///
/// let scope = IndexScope::Partition;
/// assert!(scope.is_partition());
/// assert!(!scope.is_local());
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum IndexScope {
    /// The index column is used to partition rows across nodes of a distributed database.
    Partition,

    /// The index column is scoped to a physical node.
    Local,
}

impl IndexColumn {
    /// Returns the [`Column`] referenced by this index column.
    pub fn table_column<'a>(&self, schema: &'a Schema) -> &'a Column {
        schema.column(self.column)
    }
}

impl IndexScope {
    /// Returns `true` if this is the [`Partition`](IndexScope::Partition) scope.
    pub fn is_partition(self) -> bool {
        matches!(self, Self::Partition)
    }

    /// Returns `true` if this is the [`Local`](IndexScope::Local) scope.
    pub fn is_local(self) -> bool {
        matches!(self, Self::Local)
    }
}

impl IndexId {
    pub(crate) fn placeholder() -> Self {
        Self {
            table: TableId::placeholder(),
            index: usize::MAX,
        }
    }
}

impl fmt::Debug for IndexId {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "IndexId({}/{})", self.table.0, self.index)
    }
}

#[cfg(all(test, feature = "serde"))]
mod serde_tests {
    use crate::schema::db::{ColumnId, Index, IndexColumn, IndexId, IndexOp, IndexScope, TableId};

    fn base_index() -> Index {
        Index {
            id: IndexId {
                table: TableId(0),
                index: 0,
            },
            name: "idx".to_string(),
            on: TableId(0),
            columns: vec![IndexColumn {
                column: ColumnId {
                    table: TableId(0),
                    index: 0,
                },
                op: IndexOp::Eq,
                scope: IndexScope::Local,
            }],
            unique: false,
            primary_key: false,
        }
    }

    #[test]
    fn false_booleans_are_omitted() {
        let toml = toml::to_string(&base_index()).unwrap();
        assert!(!toml.contains("unique"), "toml: {toml}");
        assert!(!toml.contains("primary_key"), "toml: {toml}");
    }

    #[test]
    fn unique_true_is_included() {
        let idx = Index {
            unique: true,
            ..base_index()
        };
        let toml = toml::to_string(&idx).unwrap();
        assert!(toml.contains("unique = true"), "toml: {toml}");
    }

    #[test]
    fn primary_key_true_is_included() {
        let idx = Index {
            primary_key: true,
            ..base_index()
        };
        let toml = toml::to_string(&idx).unwrap();
        assert!(toml.contains("primary_key = true"), "toml: {toml}");
    }

    #[test]
    fn missing_bool_fields_deserialize_as_false() {
        let toml = "name = \"idx\"\non = 0\n\n[id]\ntable = 0\nindex = 0\n\n[[columns]]\nop = \"Eq\"\nscope = \"Local\"\n\n[columns.column]\ntable = 0\nindex = 0\n";
        let idx: Index = toml::from_str(toml).unwrap();
        assert!(!idx.unique);
        assert!(!idx.primary_key);
    }

    #[test]
    fn round_trip_all_true() {
        let original = Index {
            unique: true,
            primary_key: true,
            ..base_index()
        };
        let decoded: Index = toml::from_str(&toml::to_string(&original).unwrap()).unwrap();
        assert_eq!(decoded.unique, original.unique);
        assert_eq!(decoded.primary_key, original.primary_key);
        assert_eq!(decoded.name, original.name);
    }
}

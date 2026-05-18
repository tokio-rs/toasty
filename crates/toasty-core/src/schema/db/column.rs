use super::{TableId, Type, table};
use crate::stmt;

use std::fmt;

/// A column in a database table.
///
/// Each column has a logical type ([`stmt::Type`]) used by the query engine and
/// a storage type ([`Type`]) representing how the value is stored in the database.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::schema::db::{Column, ColumnId, TableId, Type};
/// use toasty_core::stmt;
///
/// let column = Column {
///     id: ColumnId { table: TableId(0), index: 0 },
///     name: "email".to_string(),
///     ty: stmt::Type::String,
///     storage_ty: Type::VarChar(255),
///     nullable: false,
///     primary_key: false,
///     auto_increment: false,
/// };
///
/// assert_eq!(column.name, "email");
/// assert!(!column.nullable);
/// ```
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Column {
    /// Uniquely identifies the column in the schema.
    pub id: ColumnId,

    /// The name of the column in the database.
    pub name: String,

    /// The column type, from Toasty's point of view.
    pub ty: stmt::Type,

    /// The database storage type of the column.
    pub storage_ty: Type,

    /// Whether or not the column is nullable
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "is_false"))]
    pub nullable: bool,

    /// True if the column is part of the table's primary key
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "is_false"))]
    pub primary_key: bool,

    /// True if the column is an integer that should be auto-incremented
    /// with each insertion of a new row. This should be false if a `storage_ty`
    /// of type `Serial` is used.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "is_false"))]
    pub auto_increment: bool,

    /// True if the column tracks an OCC version counter.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "is_false"))]
    pub versionable: bool,
}

#[cfg(feature = "serde")]
fn is_false(b: &bool) -> bool {
    !*b
}

/// Uniquely identifies a column within a schema.
///
/// A `ColumnId` combines the [`TableId`] of the owning table with the column's
/// positional index within that table's column list.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::schema::db::{ColumnId, TableId};
///
/// let id = ColumnId { table: TableId(0), index: 2 };
/// assert_eq!(id.index, 2);
/// ```
#[derive(PartialEq, Eq, Clone, Copy, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ColumnId {
    /// The table this column belongs to.
    pub table: TableId,
    /// Zero-based position of this column in the table's column list.
    pub index: usize,
}

impl ColumnId {
    pub(crate) fn placeholder() -> Self {
        Self {
            table: table::TableId::placeholder(),
            index: usize::MAX,
        }
    }
}

impl From<&Column> for ColumnId {
    fn from(value: &Column) -> Self {
        value.id
    }
}

impl fmt::Debug for ColumnId {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "ColumnId({}/{})", self.table.0, self.index)
    }
}

#[cfg(all(test, feature = "serde"))]
mod serde_tests {
    use crate::schema::db::{Column, ColumnId, TableId, Type};
    use crate::stmt;

    fn base_column() -> Column {
        Column {
            id: ColumnId {
                table: TableId(0),
                index: 0,
            },
            name: "test".to_string(),
            ty: stmt::Type::String,
            storage_ty: Type::Text,
            nullable: false,
            primary_key: false,
            auto_increment: false,
            versionable: false,
        }
    }

    #[test]
    fn false_booleans_are_omitted() {
        let toml = toml::to_string(&base_column()).unwrap();
        assert!(!toml.contains("nullable"), "toml: {toml}");
        assert!(!toml.contains("primary_key"), "toml: {toml}");
        assert!(!toml.contains("auto_increment"), "toml: {toml}");
        assert!(!toml.contains("versionable"), "toml: {toml}");
    }

    #[test]
    fn nullable_true_is_included() {
        let col = Column {
            nullable: true,
            ..base_column()
        };
        let toml = toml::to_string(&col).unwrap();
        assert!(toml.contains("nullable = true"), "toml: {toml}");
    }

    #[test]
    fn primary_key_true_is_included() {
        let col = Column {
            primary_key: true,
            ..base_column()
        };
        let toml = toml::to_string(&col).unwrap();
        assert!(toml.contains("primary_key = true"), "toml: {toml}");
    }

    #[test]
    fn auto_increment_true_is_included() {
        let col = Column {
            auto_increment: true,
            ..base_column()
        };
        let toml = toml::to_string(&col).unwrap();
        assert!(toml.contains("auto_increment = true"), "toml: {toml}");
    }

    #[test]
    fn missing_bool_fields_deserialize_as_false() {
        let toml = "name = \"test\"\nty = \"String\"\nstorage_ty = \"Text\"\n\n[id]\ntable = 0\nindex = 0\n";
        let col: Column = toml::from_str(toml).unwrap();
        assert!(!col.nullable);
        assert!(!col.primary_key);
        assert!(!col.auto_increment);
        assert!(!col.versionable);
    }

    #[test]
    fn round_trip_all_true() {
        let original = Column {
            nullable: true,
            primary_key: true,
            auto_increment: true,
            ..base_column()
        };
        let decoded: Column = toml::from_str(&toml::to_string(&original).unwrap()).unwrap();
        assert_eq!(original, decoded);
    }
}

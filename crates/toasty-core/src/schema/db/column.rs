use super::{DiffContext, TableId, Type, table};
use crate::stmt;

use std::{
    collections::{HashMap, HashSet},
    fmt,
    ops::Deref,
};

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

/// The set of differences between two column lists.
///
/// Computed by [`ColumnsDiff::from`] and dereferences to
/// `Vec<ColumnsDiffItem>` for iteration.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::schema::db::{ColumnsDiff, DiffContext, RenameHints, Schema};
///
/// let previous = Schema::default();
/// let next = Schema::default();
/// let hints = RenameHints::new();
/// let cx = DiffContext::new(&previous, &next, &hints);
/// let diff = ColumnsDiff::from(&cx, &[], &[]);
/// assert!(diff.is_empty());
/// ```
pub struct ColumnsDiff<'a> {
    items: Vec<ColumnsDiffItem<'a>>,
}

impl<'a> ColumnsDiff<'a> {
    /// Computes the diff between two column slices.
    ///
    /// Uses [`DiffContext`] to resolve rename hints. Columns matched by name
    /// (or by rename hint) are compared field-by-field; unmatched columns in
    /// `previous` become drops, and unmatched columns in `next` become adds.
    pub fn from(cx: &DiffContext<'a>, previous: &'a [Column], next: &'a [Column]) -> Self {
        fn has_diff(previous: &Column, next: &Column) -> bool {
            previous.name != next.name
                || previous.storage_ty != next.storage_ty
                || previous.nullable != next.nullable
                || previous.primary_key != next.primary_key
                || previous.auto_increment != next.auto_increment
                || previous.versionable != next.versionable
        }

        let mut items = vec![];
        let mut add_ids: HashSet<_> = next.iter().map(|next| next.id).collect();

        let next_map =
            HashMap::<&str, &'a Column>::from_iter(next.iter().map(|to| (to.name.as_str(), to)));

        for previous in previous {
            let next = if let Some(next_id) = cx.rename_hints().get_column(previous.id) {
                cx.next().column(next_id)
            } else if let Some(next) = next_map.get(previous.name.as_str()) {
                next
            } else {
                items.push(ColumnsDiffItem::DropColumn(previous));
                continue;
            };

            add_ids.remove(&next.id);

            if has_diff(previous, next) {
                items.push(ColumnsDiffItem::AlterColumn { previous, next });
            }
        }

        for column_id in add_ids {
            items.push(ColumnsDiffItem::AddColumn(cx.next().column(column_id)));
        }

        Self { items }
    }

    /// Returns `true` if there are no column changes.
    pub const fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl<'a> Deref for ColumnsDiff<'a> {
    type Target = Vec<ColumnsDiffItem<'a>>;

    fn deref(&self) -> &Self::Target {
        &self.items
    }
}

/// A single change detected between two column lists.
pub enum ColumnsDiffItem<'a> {
    /// A new column was added.
    AddColumn(&'a Column),
    /// An existing column was removed.
    DropColumn(&'a Column),
    /// A column was modified (name, type, nullability, or other property changed).
    AlterColumn {
        /// The column definition before the change.
        previous: &'a Column,
        /// The column definition after the change.
        next: &'a Column,
    },
}

#[cfg(test)]
mod tests {
    use crate::schema::db::{
        Column, ColumnId, ColumnsDiff, ColumnsDiffItem, DiffContext, PrimaryKey, RenameHints,
        Schema, Table, TableId, Type,
    };
    use crate::stmt;

    fn make_column(
        table_id: usize,
        index: usize,
        name: &str,
        storage_ty: Type,
        nullable: bool,
    ) -> Column {
        Column {
            id: ColumnId {
                table: TableId(table_id),
                index,
            },
            name: name.to_string(),
            ty: stmt::Type::String, // Simplified for tests
            storage_ty,
            nullable,
            primary_key: false,
            auto_increment: false,
            versionable: false,
        }
    }

    fn make_schema_with_columns(table_id: usize, columns: Vec<Column>) -> Schema {
        let mut schema = Schema::default();
        schema.tables.push(Table {
            id: TableId(table_id),
            name: "test_table".to_string(),
            columns,
            primary_key: PrimaryKey {
                columns: vec![],
                index: super::super::IndexId {
                    table: TableId(table_id),
                    index: 0,
                },
            },
            indices: vec![],
        });
        schema
    }

    #[test]
    fn test_no_diff_same_columns() {
        let from_cols = vec![
            make_column(0, 0, "id", Type::Integer(8), false),
            make_column(0, 1, "name", Type::Text, false),
        ];
        let to_cols = vec![
            make_column(0, 0, "id", Type::Integer(8), false),
            make_column(0, 1, "name", Type::Text, false),
        ];

        let from_schema = make_schema_with_columns(0, from_cols.clone());
        let to_schema = make_schema_with_columns(0, to_cols.clone());
        let hints = RenameHints::new();
        let cx = DiffContext::new(&from_schema, &to_schema, &hints);

        let diff = ColumnsDiff::from(&cx, &from_cols, &to_cols);
        assert!(diff.is_empty());
    }

    #[test]
    fn test_add_column() {
        let from_cols = vec![make_column(0, 0, "id", Type::Integer(8), false)];
        let to_cols = vec![
            make_column(0, 0, "id", Type::Integer(8), false),
            make_column(0, 1, "name", Type::Text, false),
        ];

        let from_schema = make_schema_with_columns(0, from_cols.clone());
        let to_schema = make_schema_with_columns(0, to_cols.clone());
        let hints = RenameHints::new();
        let cx = DiffContext::new(&from_schema, &to_schema, &hints);

        let diff = ColumnsDiff::from(&cx, &from_cols, &to_cols);
        assert_eq!(diff.items.len(), 1);
        assert!(matches!(diff.items[0], ColumnsDiffItem::AddColumn(_)));
        if let ColumnsDiffItem::AddColumn(col) = diff.items[0] {
            assert_eq!(col.name, "name");
        }
    }

    #[test]
    fn test_drop_column() {
        let from_cols = vec![
            make_column(0, 0, "id", Type::Integer(8), false),
            make_column(0, 1, "name", Type::Text, false),
        ];
        let to_cols = vec![make_column(0, 0, "id", Type::Integer(8), false)];

        let from_schema = make_schema_with_columns(0, from_cols.clone());
        let to_schema = make_schema_with_columns(0, to_cols.clone());
        let hints = RenameHints::new();
        let cx = DiffContext::new(&from_schema, &to_schema, &hints);

        let diff = ColumnsDiff::from(&cx, &from_cols, &to_cols);
        assert_eq!(diff.items.len(), 1);
        assert!(matches!(diff.items[0], ColumnsDiffItem::DropColumn(_)));
        if let ColumnsDiffItem::DropColumn(col) = diff.items[0] {
            assert_eq!(col.name, "name");
        }
    }

    #[test]
    fn test_alter_column_type() {
        let from_cols = vec![make_column(0, 0, "id", Type::Integer(8), false)];
        let to_cols = vec![make_column(0, 0, "id", Type::Text, false)];

        let from_schema = make_schema_with_columns(0, from_cols.clone());
        let to_schema = make_schema_with_columns(0, to_cols.clone());
        let hints = RenameHints::new();
        let cx = DiffContext::new(&from_schema, &to_schema, &hints);

        let diff = ColumnsDiff::from(&cx, &from_cols, &to_cols);
        assert_eq!(diff.items.len(), 1);
        assert!(matches!(diff.items[0], ColumnsDiffItem::AlterColumn { .. }));
    }

    #[test]
    fn test_alter_column_nullable() {
        let from_cols = vec![make_column(0, 0, "id", Type::Integer(8), false)];
        let to_cols = vec![make_column(0, 0, "id", Type::Integer(8), true)];

        let from_schema = make_schema_with_columns(0, from_cols.clone());
        let to_schema = make_schema_with_columns(0, to_cols.clone());
        let hints = RenameHints::new();
        let cx = DiffContext::new(&from_schema, &to_schema, &hints);

        let diff = ColumnsDiff::from(&cx, &from_cols, &to_cols);
        assert_eq!(diff.items.len(), 1);
        assert!(matches!(diff.items[0], ColumnsDiffItem::AlterColumn { .. }));
    }

    #[test]
    fn test_rename_column_with_hint() {
        // Column renamed from "old_name" to "new_name"
        let from_cols = vec![make_column(0, 0, "old_name", Type::Text, false)];
        let to_cols = vec![make_column(0, 0, "new_name", Type::Text, false)];

        let from_schema = make_schema_with_columns(0, from_cols.clone());
        let to_schema = make_schema_with_columns(0, to_cols.clone());

        let mut hints = RenameHints::new();
        hints.add_column_hint(
            ColumnId {
                table: TableId(0),
                index: 0,
            },
            ColumnId {
                table: TableId(0),
                index: 0,
            },
        );
        let cx = DiffContext::new(&from_schema, &to_schema, &hints);

        let diff = ColumnsDiff::from(&cx, &from_cols, &to_cols);
        assert_eq!(diff.items.len(), 1);
        assert!(matches!(diff.items[0], ColumnsDiffItem::AlterColumn { .. }));
        if let ColumnsDiffItem::AlterColumn { previous, next } = diff.items[0] {
            assert_eq!(previous.name, "old_name");
            assert_eq!(next.name, "new_name");
        }
    }

    #[test]
    fn test_rename_column_without_hint_is_drop_and_add() {
        // Column renamed from "old_name" to "new_name", but no hint provided
        // Should be treated as drop + add
        let from_cols = vec![make_column(0, 0, "old_name", Type::Text, false)];
        let to_cols = vec![make_column(0, 0, "new_name", Type::Text, false)];

        let from_schema = make_schema_with_columns(0, from_cols.clone());
        let to_schema = make_schema_with_columns(0, to_cols.clone());
        let hints = RenameHints::new();
        let cx = DiffContext::new(&from_schema, &to_schema, &hints);

        let diff = ColumnsDiff::from(&cx, &from_cols, &to_cols);
        assert_eq!(diff.items.len(), 2);

        let has_drop = diff
            .items
            .iter()
            .any(|item| matches!(item, ColumnsDiffItem::DropColumn(_)));
        let has_add = diff
            .items
            .iter()
            .any(|item| matches!(item, ColumnsDiffItem::AddColumn(_)));
        assert!(has_drop);
        assert!(has_add);
    }

    #[cfg(feature = "serde")]
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

    #[test]
    fn test_multiple_operations() {
        let from_cols = vec![
            make_column(0, 0, "id", Type::Integer(8), false),
            make_column(0, 1, "old_name", Type::Text, false),
            make_column(0, 2, "to_drop", Type::Text, false),
        ];
        let to_cols = vec![
            make_column(0, 0, "id", Type::Text, false), // type changed
            make_column(0, 1, "new_name", Type::Text, false), // renamed
            make_column(0, 2, "added", Type::Integer(8), false), // new column
        ];

        let from_schema = make_schema_with_columns(0, from_cols.clone());
        let to_schema = make_schema_with_columns(0, to_cols.clone());

        let mut hints = RenameHints::new();
        hints.add_column_hint(
            ColumnId {
                table: TableId(0),
                index: 1,
            },
            ColumnId {
                table: TableId(0),
                index: 1,
            },
        );
        let cx = DiffContext::new(&from_schema, &to_schema, &hints);

        let diff = ColumnsDiff::from(&cx, &from_cols, &to_cols);
        // Should have: 1 alter (id type changed), 1 alter (renamed), 1 drop (to_drop), 1 add (added)
        assert_eq!(diff.items.len(), 4);
    }
}

use super::{table, DiffContext, TableId, Type};
use crate::stmt;

use std::{
    collections::{HashMap, HashSet},
    fmt,
    ops::Deref,
};

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
    pub nullable: bool,

    /// True if the column is part of the table's primary key
    pub primary_key: bool,

    /// True if the column is an integer that should be auto-incremented
    /// with each insertion of a new row. This should be false if a `storage_ty`
    /// of type `Serial` is used.
    pub auto_increment: bool,
}

#[derive(PartialEq, Eq, Clone, Copy, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ColumnId {
    pub table: TableId,
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

pub struct ColumnsDiff<'a> {
    items: Vec<ColumnsDiffItem<'a>>,
}

impl<'a> ColumnsDiff<'a> {
    pub fn from(cx: &DiffContext<'a>, from: &'a [Column], to: &'a [Column]) -> Self {
        fn has_diff(from: &Column, to: &Column) -> bool {
            from.name != to.name
                || from.storage_ty != to.storage_ty
                || from.nullable != to.nullable
                || from.primary_key != to.primary_key
                || from.auto_increment != to.auto_increment
        }

        let mut items = vec![];
        let mut add_ids: HashSet<_> = to.iter().map(|to| to.id).collect();

        let to_map =
            HashMap::<&str, &'a Column>::from_iter(to.iter().map(|to| (to.name.as_str(), to)));

        for from in from {
            let to = if let Some(to_id) = cx.rename_hints().get_column(from.id) {
                cx.schema_to().column(to_id)
            } else if let Some(to) = to_map.get(from.name.as_str()) {
                to
            } else {
                items.push(ColumnsDiffItem::DropColumn(from));
                continue;
            };

            add_ids.remove(&to.id);

            if has_diff(from, to) {
                items.push(ColumnsDiffItem::AlterColumn { from, to });
            }
        }

        for column_id in add_ids {
            items.push(ColumnsDiffItem::AddColumn(cx.schema_to().column(column_id)));
        }

        Self { items }
    }

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

pub enum ColumnsDiffItem<'a> {
    AddColumn(&'a Column),
    DropColumn(&'a Column),
    AlterColumn { from: &'a Column, to: &'a Column },
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
        if let ColumnsDiffItem::AlterColumn { from, to } = diff.items[0] {
            assert_eq!(from.name, "old_name");
            assert_eq!(to.name, "new_name");
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

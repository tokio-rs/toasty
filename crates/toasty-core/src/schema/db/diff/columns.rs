use super::Context;
use crate::schema::db::Column;

use hashbrown::{HashMap, HashSet};
use std::ops::Deref;

/// The set of differences between two column lists.
///
/// Computed by [`Columns::from`] and dereferences to `Vec<ColumnsItem>` for
/// iteration.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::schema::db::{Schema, diff};
///
/// let previous = Schema::default();
/// let next = Schema::default();
/// let hints = diff::RenameHints::new();
/// let cx = diff::Context::new(&previous, &next, &hints);
/// let d = diff::Columns::from(&cx, &[], &[]);
/// assert!(d.is_empty());
/// ```
pub struct Columns<'a> {
    items: Vec<ColumnsItem<'a>>,
}

impl<'a> Columns<'a> {
    /// Computes the diff between two column slices.
    ///
    /// Uses [`Context`] to resolve rename hints. Columns matched by name (or
    /// by rename hint) are compared field-by-field; unmatched columns in
    /// `previous` become drops, and unmatched columns in `next` become adds.
    pub fn from(cx: &Context<'a>, previous: &'a [Column], next: &'a [Column]) -> Self {
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
                items.push(ColumnsItem::DropColumn(previous));
                continue;
            };

            add_ids.remove(&next.id);

            if has_diff(previous, next) {
                items.push(ColumnsItem::AlterColumn { previous, next });
            }
        }

        for column_id in add_ids {
            items.push(ColumnsItem::AddColumn(cx.next().column(column_id)));
        }

        Self { items }
    }

    /// Returns `true` if there are no column changes.
    pub const fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl<'a> Deref for Columns<'a> {
    type Target = Vec<ColumnsItem<'a>>;

    fn deref(&self) -> &Self::Target {
        &self.items
    }
}

/// A single change detected between two column lists.
pub enum ColumnsItem<'a> {
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
        Column, ColumnId, IndexId, PrimaryKey, Schema, Table, TableId, Type,
        diff::{self, Columns, ColumnsItem},
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
            ty: stmt::Type::String,
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
                index: IndexId {
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
        let hints = diff::RenameHints::new();
        let cx = diff::Context::new(&from_schema, &to_schema, &hints);

        let d = Columns::from(&cx, &from_cols, &to_cols);
        assert!(d.is_empty());
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
        let hints = diff::RenameHints::new();
        let cx = diff::Context::new(&from_schema, &to_schema, &hints);

        let d = Columns::from(&cx, &from_cols, &to_cols);
        assert_eq!(d.len(), 1);
        assert!(matches!(d[0], ColumnsItem::AddColumn(_)));
        if let ColumnsItem::AddColumn(col) = d[0] {
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
        let hints = diff::RenameHints::new();
        let cx = diff::Context::new(&from_schema, &to_schema, &hints);

        let d = Columns::from(&cx, &from_cols, &to_cols);
        assert_eq!(d.len(), 1);
        assert!(matches!(d[0], ColumnsItem::DropColumn(_)));
        if let ColumnsItem::DropColumn(col) = d[0] {
            assert_eq!(col.name, "name");
        }
    }

    #[test]
    fn test_alter_column_type() {
        let from_cols = vec![make_column(0, 0, "id", Type::Integer(8), false)];
        let to_cols = vec![make_column(0, 0, "id", Type::Text, false)];

        let from_schema = make_schema_with_columns(0, from_cols.clone());
        let to_schema = make_schema_with_columns(0, to_cols.clone());
        let hints = diff::RenameHints::new();
        let cx = diff::Context::new(&from_schema, &to_schema, &hints);

        let d = Columns::from(&cx, &from_cols, &to_cols);
        assert_eq!(d.len(), 1);
        assert!(matches!(d[0], ColumnsItem::AlterColumn { .. }));
    }

    #[test]
    fn test_alter_column_nullable() {
        let from_cols = vec![make_column(0, 0, "id", Type::Integer(8), false)];
        let to_cols = vec![make_column(0, 0, "id", Type::Integer(8), true)];

        let from_schema = make_schema_with_columns(0, from_cols.clone());
        let to_schema = make_schema_with_columns(0, to_cols.clone());
        let hints = diff::RenameHints::new();
        let cx = diff::Context::new(&from_schema, &to_schema, &hints);

        let d = Columns::from(&cx, &from_cols, &to_cols);
        assert_eq!(d.len(), 1);
        assert!(matches!(d[0], ColumnsItem::AlterColumn { .. }));
    }

    #[test]
    fn test_rename_column_with_hint() {
        let from_cols = vec![make_column(0, 0, "old_name", Type::Text, false)];
        let to_cols = vec![make_column(0, 0, "new_name", Type::Text, false)];

        let from_schema = make_schema_with_columns(0, from_cols.clone());
        let to_schema = make_schema_with_columns(0, to_cols.clone());

        let mut hints = diff::RenameHints::new();
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
        let cx = diff::Context::new(&from_schema, &to_schema, &hints);

        let d = Columns::from(&cx, &from_cols, &to_cols);
        assert_eq!(d.len(), 1);
        assert!(matches!(d[0], ColumnsItem::AlterColumn { .. }));
        if let ColumnsItem::AlterColumn { previous, next } = d[0] {
            assert_eq!(previous.name, "old_name");
            assert_eq!(next.name, "new_name");
        }
    }

    #[test]
    fn test_rename_column_without_hint_is_drop_and_add() {
        let from_cols = vec![make_column(0, 0, "old_name", Type::Text, false)];
        let to_cols = vec![make_column(0, 0, "new_name", Type::Text, false)];

        let from_schema = make_schema_with_columns(0, from_cols.clone());
        let to_schema = make_schema_with_columns(0, to_cols.clone());
        let hints = diff::RenameHints::new();
        let cx = diff::Context::new(&from_schema, &to_schema, &hints);

        let d = Columns::from(&cx, &from_cols, &to_cols);
        assert_eq!(d.len(), 2);

        let has_drop = d
            .iter()
            .any(|item| matches!(item, ColumnsItem::DropColumn(_)));
        let has_add = d
            .iter()
            .any(|item| matches!(item, ColumnsItem::AddColumn(_)));
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
            make_column(0, 0, "id", Type::Text, false),
            make_column(0, 1, "new_name", Type::Text, false),
            make_column(0, 2, "added", Type::Integer(8), false),
        ];

        let from_schema = make_schema_with_columns(0, from_cols.clone());
        let to_schema = make_schema_with_columns(0, to_cols.clone());

        let mut hints = diff::RenameHints::new();
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
        let cx = diff::Context::new(&from_schema, &to_schema, &hints);

        let d = Columns::from(&cx, &from_cols, &to_cols);
        assert_eq!(d.len(), 4);
    }
}

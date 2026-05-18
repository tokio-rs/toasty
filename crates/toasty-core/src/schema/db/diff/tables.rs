use super::{Columns, Context, Indices};
use crate::schema::db::Table;

use hashbrown::{HashMap, HashSet};
use std::ops::Deref;

/// The set of differences between two table lists.
///
/// Computed by [`Tables::from`] and dereferences to `Vec<TablesItem>` for
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
/// let d = diff::Tables::from(&cx, &[], &[]);
/// assert!(d.is_empty());
/// ```
pub struct Tables<'a> {
    items: Vec<TablesItem<'a>>,
}

impl<'a> Tables<'a> {
    /// Computes the diff between two table slices.
    ///
    /// Uses [`Context`] to resolve rename hints. Tables matched by name (or
    /// by rename hint) are compared for column and index changes; unmatched
    /// tables in `previous` become drops, and unmatched tables in `next`
    /// become creates.
    pub fn from(cx: &Context<'a>, previous: &'a [Table], next: &'a [Table]) -> Self {
        let mut items = vec![];
        let mut create_ids: HashSet<_> = next.iter().map(|next| next.id).collect();

        let next_map = HashMap::<&str, &'a Table>::from_iter(
            next.iter().map(|next| (next.name.as_str(), next)),
        );

        for previous in previous {
            let next = if let Some(next_id) = cx.rename_hints().get_table(previous.id) {
                cx.next().table(next_id)
            } else if let Some(to) = next_map.get(previous.name.as_str()) {
                to
            } else {
                items.push(TablesItem::DropTable(previous));
                continue;
            };

            create_ids.remove(&next.id);

            let columns = Columns::from(cx, &previous.columns, &next.columns);
            let indices = Indices::from(cx, &previous.indices, &next.indices);
            if previous.name != next.name || !columns.is_empty() || !indices.is_empty() {
                items.push(TablesItem::AlterTable {
                    previous,
                    next,
                    columns,
                    indices,
                });
            }
        }

        for table_id in create_ids {
            items.push(TablesItem::CreateTable(cx.next().table(table_id)));
        }

        Self { items }
    }
}

impl<'a> Deref for Tables<'a> {
    type Target = Vec<TablesItem<'a>>;

    fn deref(&self) -> &Self::Target {
        &self.items
    }
}

/// A single change detected between two table lists.
pub enum TablesItem<'a> {
    /// A new table was created.
    CreateTable(&'a Table),
    /// An existing table was dropped.
    DropTable(&'a Table),
    /// A table was modified (name, columns, or indices changed).
    AlterTable {
        /// The table definition before the change.
        previous: &'a Table,
        /// The table definition after the change.
        next: &'a Table,
        /// Column-level changes within this table.
        columns: Columns<'a>,
        /// Index-level changes within this table.
        indices: Indices<'a>,
    },
}

#[cfg(test)]
mod tests {
    use crate::schema::db::{
        Column, ColumnId, IndexId, PrimaryKey, Schema, Table, TableId, Type,
        diff::{self, Tables, TablesItem},
    };
    use crate::stmt;

    fn make_table(id: usize, name: &str, num_columns: usize) -> Table {
        let mut columns = vec![];
        for i in 0..num_columns {
            columns.push(Column {
                id: ColumnId {
                    table: TableId(id),
                    index: i,
                },
                name: format!("col{}", i),
                ty: stmt::Type::String,
                storage_ty: Type::Text,
                nullable: false,
                primary_key: false,
                auto_increment: false,
                versionable: false,
            });
        }

        Table {
            id: TableId(id),
            name: name.to_string(),
            columns,
            primary_key: PrimaryKey {
                columns: vec![],
                index: IndexId {
                    table: TableId(id),
                    index: 0,
                },
            },
            indices: vec![],
        }
    }

    fn make_schema(tables: Vec<Table>) -> Schema {
        Schema { tables }
    }

    #[test]
    fn test_no_diff_same_tables() {
        let from_tables = vec![make_table(0, "users", 2), make_table(1, "posts", 3)];
        let to_tables = vec![make_table(0, "users", 2), make_table(1, "posts", 3)];

        let from_schema = make_schema(from_tables.clone());
        let to_schema = make_schema(to_tables.clone());
        let hints = diff::RenameHints::new();
        let cx = diff::Context::new(&from_schema, &to_schema, &hints);

        let d = Tables::from(&cx, &from_tables, &to_tables);
        assert_eq!(d.len(), 0);
    }

    #[test]
    fn test_create_table() {
        let from_tables = vec![make_table(0, "users", 2)];
        let to_tables = vec![make_table(0, "users", 2), make_table(1, "posts", 3)];

        let from_schema = make_schema(from_tables.clone());
        let to_schema = make_schema(to_tables.clone());
        let hints = diff::RenameHints::new();
        let cx = diff::Context::new(&from_schema, &to_schema, &hints);

        let d = Tables::from(&cx, &from_tables, &to_tables);
        assert_eq!(d.len(), 1);
        assert!(matches!(d[0], TablesItem::CreateTable(_)));
        if let TablesItem::CreateTable(table) = d[0] {
            assert_eq!(table.name, "posts");
        }
    }

    #[test]
    fn test_drop_table() {
        let from_tables = vec![make_table(0, "users", 2), make_table(1, "posts", 3)];
        let to_tables = vec![make_table(0, "users", 2)];

        let from_schema = make_schema(from_tables.clone());
        let to_schema = make_schema(to_tables.clone());
        let hints = diff::RenameHints::new();
        let cx = diff::Context::new(&from_schema, &to_schema, &hints);

        let d = Tables::from(&cx, &from_tables, &to_tables);
        assert_eq!(d.len(), 1);
        assert!(matches!(d[0], TablesItem::DropTable(_)));
        if let TablesItem::DropTable(table) = d[0] {
            assert_eq!(table.name, "posts");
        }
    }

    #[test]
    fn test_rename_table_with_hint() {
        let from_tables = vec![make_table(0, "old_users", 2)];
        let to_tables = vec![make_table(0, "new_users", 2)];

        let from_schema = make_schema(from_tables.clone());
        let to_schema = make_schema(to_tables.clone());

        let mut hints = diff::RenameHints::new();
        hints.add_table_hint(TableId(0), TableId(0));
        let cx = diff::Context::new(&from_schema, &to_schema, &hints);

        let d = Tables::from(&cx, &from_tables, &to_tables);
        assert_eq!(d.len(), 1);
        assert!(matches!(d[0], TablesItem::AlterTable { .. }));
        if let TablesItem::AlterTable { previous, next, .. } = &d[0] {
            assert_eq!(previous.name, "old_users");
            assert_eq!(next.name, "new_users");
        }
    }

    #[test]
    fn test_rename_table_without_hint_is_drop_and_create() {
        let from_tables = vec![make_table(0, "old_users", 2)];
        let to_tables = vec![make_table(0, "new_users", 2)];

        let from_schema = make_schema(from_tables.clone());
        let to_schema = make_schema(to_tables.clone());
        let hints = diff::RenameHints::new();
        let cx = diff::Context::new(&from_schema, &to_schema, &hints);

        let d = Tables::from(&cx, &from_tables, &to_tables);
        assert_eq!(d.len(), 2);

        let has_drop = d
            .iter()
            .any(|item| matches!(item, TablesItem::DropTable(_)));
        let has_create = d
            .iter()
            .any(|item| matches!(item, TablesItem::CreateTable(_)));
        assert!(has_drop);
        assert!(has_create);
    }

    #[test]
    fn test_alter_table_column_change() {
        let from_tables = vec![make_table(0, "users", 2)];
        let to_tables = vec![make_table(0, "users", 3)];

        let from_schema = make_schema(from_tables.clone());
        let to_schema = make_schema(to_tables.clone());
        let hints = diff::RenameHints::new();
        let cx = diff::Context::new(&from_schema, &to_schema, &hints);

        let d = Tables::from(&cx, &from_tables, &to_tables);
        assert_eq!(d.len(), 1);
        assert!(matches!(d[0], TablesItem::AlterTable { .. }));
    }

    #[test]
    fn test_multiple_operations() {
        let from_tables = vec![
            make_table(0, "users", 2),
            make_table(1, "posts", 3),
            make_table(2, "old_table", 1),
        ];
        let to_tables = vec![
            make_table(0, "users", 3),
            make_table(1, "new_posts", 3),
            make_table(2, "comments", 2),
        ];

        let from_schema = make_schema(from_tables.clone());
        let to_schema = make_schema(to_tables.clone());

        let mut hints = diff::RenameHints::new();
        hints.add_table_hint(TableId(1), TableId(1));
        let cx = diff::Context::new(&from_schema, &to_schema, &hints);

        let d = Tables::from(&cx, &from_tables, &to_tables);
        assert_eq!(d.len(), 4);
    }
}

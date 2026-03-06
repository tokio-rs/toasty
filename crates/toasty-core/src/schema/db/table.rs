use super::{Column, ColumnId, Index, IndexId, PrimaryKey};
use crate::{
    schema::db::{column::ColumnsDiff, diff::DiffContext, index::IndicesDiff},
    stmt,
};

use std::{
    collections::{HashMap, HashSet},
    fmt,
    ops::Deref,
};

/// A database table
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Table {
    /// Uniquely identifies a table
    pub id: TableId,

    /// Name of the table
    pub name: String,

    /// The table's columns
    pub columns: Vec<Column>,

    pub primary_key: PrimaryKey,

    pub indices: Vec<Index>,
}

/// Uniquely identifies a table
#[derive(PartialEq, Eq, Clone, Copy, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TableId(pub usize);

impl Table {
    pub fn primary_key_column(&self, i: usize) -> &Column {
        &self.columns[self.primary_key.columns[i].index]
    }

    pub fn primary_key_columns(&self) -> impl ExactSizeIterator<Item = &Column> + '_ {
        self.primary_key
            .columns
            .iter()
            .map(|column_id| &self.columns[column_id.index])
    }

    pub fn column(&self, id: impl Into<ColumnId>) -> &Column {
        &self.columns[id.into().index]
    }

    /// The path must have exactly one step
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

pub struct TablesDiff<'a> {
    items: Vec<TablesDiffItem<'a>>,
}

impl<'a> TablesDiff<'a> {
    pub fn from(cx: &DiffContext<'a>, previous: &'a [Table], next: &'a [Table]) -> Self {
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
                items.push(TablesDiffItem::DropTable(previous));
                continue;
            };

            create_ids.remove(&next.id);

            let columns = ColumnsDiff::from(cx, &previous.columns, &next.columns);
            let indices = IndicesDiff::from(cx, &previous.indices, &next.indices);
            if previous.name != next.name || !columns.is_empty() || !indices.is_empty() {
                items.push(TablesDiffItem::AlterTable {
                    previous,
                    next,
                    columns,
                    indices,
                });
            }
        }

        for table_id in create_ids {
            items.push(TablesDiffItem::CreateTable(cx.next().table(table_id)));
        }

        Self { items }
    }
}

impl<'a> Deref for TablesDiff<'a> {
    type Target = Vec<TablesDiffItem<'a>>;

    fn deref(&self) -> &Self::Target {
        &self.items
    }
}

pub enum TablesDiffItem<'a> {
    CreateTable(&'a Table),
    DropTable(&'a Table),
    AlterTable {
        previous: &'a Table,
        next: &'a Table,
        columns: ColumnsDiff<'a>,
        indices: IndicesDiff<'a>,
    },
}

#[cfg(test)]
mod tests {
    use crate::schema::db::{
        Column, ColumnId, DiffContext, IndexId, PrimaryKey, RenameHints, Schema, Table, TableId,
        TablesDiff, TablesDiffItem, Type,
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
        let hints = RenameHints::new();
        let cx = DiffContext::new(&from_schema, &to_schema, &hints);

        let diff = TablesDiff::from(&cx, &from_tables, &to_tables);
        assert_eq!(diff.items.len(), 0);
    }

    #[test]
    fn test_create_table() {
        let from_tables = vec![make_table(0, "users", 2)];
        let to_tables = vec![make_table(0, "users", 2), make_table(1, "posts", 3)];

        let from_schema = make_schema(from_tables.clone());
        let to_schema = make_schema(to_tables.clone());
        let hints = RenameHints::new();
        let cx = DiffContext::new(&from_schema, &to_schema, &hints);

        let diff = TablesDiff::from(&cx, &from_tables, &to_tables);
        assert_eq!(diff.items.len(), 1);
        assert!(matches!(diff.items[0], TablesDiffItem::CreateTable(_)));
        if let TablesDiffItem::CreateTable(table) = diff.items[0] {
            assert_eq!(table.name, "posts");
        }
    }

    #[test]
    fn test_drop_table() {
        let from_tables = vec![make_table(0, "users", 2), make_table(1, "posts", 3)];
        let to_tables = vec![make_table(0, "users", 2)];

        let from_schema = make_schema(from_tables.clone());
        let to_schema = make_schema(to_tables.clone());
        let hints = RenameHints::new();
        let cx = DiffContext::new(&from_schema, &to_schema, &hints);

        let diff = TablesDiff::from(&cx, &from_tables, &to_tables);
        assert_eq!(diff.items.len(), 1);
        assert!(matches!(diff.items[0], TablesDiffItem::DropTable(_)));
        if let TablesDiffItem::DropTable(table) = diff.items[0] {
            assert_eq!(table.name, "posts");
        }
    }

    #[test]
    fn test_rename_table_with_hint() {
        let from_tables = vec![make_table(0, "old_users", 2)];
        let to_tables = vec![make_table(0, "new_users", 2)];

        let from_schema = make_schema(from_tables.clone());
        let to_schema = make_schema(to_tables.clone());

        let mut hints = RenameHints::new();
        hints.add_table_hint(TableId(0), TableId(0));
        let cx = DiffContext::new(&from_schema, &to_schema, &hints);

        let diff = TablesDiff::from(&cx, &from_tables, &to_tables);
        assert_eq!(diff.items.len(), 1);
        assert!(matches!(diff.items[0], TablesDiffItem::AlterTable { .. }));
        if let TablesDiffItem::AlterTable { previous, next, .. } = &diff.items[0] {
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
        let hints = RenameHints::new();
        let cx = DiffContext::new(&from_schema, &to_schema, &hints);

        let diff = TablesDiff::from(&cx, &from_tables, &to_tables);
        assert_eq!(diff.items.len(), 2);

        let has_drop = diff
            .items
            .iter()
            .any(|item| matches!(item, TablesDiffItem::DropTable(_)));
        let has_create = diff
            .items
            .iter()
            .any(|item| matches!(item, TablesDiffItem::CreateTable(_)));
        assert!(has_drop);
        assert!(has_create);
    }

    #[test]
    fn test_alter_table_column_change() {
        let from_tables = vec![make_table(0, "users", 2)];
        let to_tables = vec![make_table(0, "users", 3)]; // added a column

        let from_schema = make_schema(from_tables.clone());
        let to_schema = make_schema(to_tables.clone());
        let hints = RenameHints::new();
        let cx = DiffContext::new(&from_schema, &to_schema, &hints);

        let diff = TablesDiff::from(&cx, &from_tables, &to_tables);
        assert_eq!(diff.items.len(), 1);
        assert!(matches!(diff.items[0], TablesDiffItem::AlterTable { .. }));
    }

    #[test]
    fn test_multiple_operations() {
        let from_tables = vec![
            make_table(0, "users", 2),
            make_table(1, "posts", 3),
            make_table(2, "old_table", 1),
        ];
        let to_tables = vec![
            make_table(0, "users", 3),     // added column
            make_table(1, "new_posts", 3), // renamed
            make_table(2, "comments", 2),  // new table (reused ID 2)
        ];

        let from_schema = make_schema(from_tables.clone());
        let to_schema = make_schema(to_tables.clone());

        let mut hints = RenameHints::new();
        hints.add_table_hint(TableId(1), TableId(1));
        let cx = DiffContext::new(&from_schema, &to_schema, &hints);

        let diff = TablesDiff::from(&cx, &from_tables, &to_tables);
        // Should have: 1 alter (users added column), 1 alter (posts renamed), 1 drop (old_table), 1 create (comments)
        assert_eq!(diff.items.len(), 4);
    }
}

use super::{Column, ColumnId, DiffContext, Schema, TableId};
use crate::stmt;

use std::{
    collections::{HashMap, HashSet},
    fmt,
    ops::Deref,
};

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
    pub unique: bool,

    /// When `true`, the index indexes the model's primary key fields.
    pub primary_key: bool,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IndexId {
    pub table: TableId,
    pub index: usize,
}

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

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum IndexOp {
    Eq,
    Sort(stmt::Direction),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum IndexScope {
    /// The index column is used to partition rows across nodes of a distributed database.
    Partition,

    /// The index column is scoped to a physical node.
    Local,
}

impl IndexColumn {
    pub fn table_column<'a>(&self, schema: &'a Schema) -> &'a Column {
        schema.column(self.column)
    }
}

impl IndexScope {
    pub fn is_partition(self) -> bool {
        matches!(self, Self::Partition)
    }

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

pub struct IndicesDiff<'a> {
    items: Vec<IndicesDiffItem<'a>>,
}

impl<'a> IndicesDiff<'a> {
    pub fn from(cx: &DiffContext<'a>, previous: &'a [Index], next: &'a [Index]) -> Self {
        fn has_diff(cx: &DiffContext<'_>, previous: &Index, next: &Index) -> bool {
            // Check basic properties
            if previous.name != next.name
                || previous.columns.len() != next.columns.len()
                || previous.unique != next.unique
                || previous.primary_key != next.primary_key
            {
                return true;
            }

            // Check if index columns have changed
            for (previous_col, next_col) in previous.columns.iter().zip(next.columns.iter()) {
                // Check if op or scope changed
                if previous_col.op != next_col.op || previous_col.scope != next_col.scope {
                    return true;
                }

                // Check if the column changed (accounting for renames)
                let columns_match =
                    if let Some(renamed_to) = cx.rename_hints().get_column(previous_col.column) {
                        // Column was renamed - check if it matches the target column
                        renamed_to == next_col.column
                    } else {
                        // No rename hint - check if columns match by name
                        let previous_column = cx.previous().column(previous_col.column);
                        let next_column = cx.next().column(next_col.column);
                        previous_column.name == next_column.name
                    };

                if !columns_match {
                    return true;
                }
            }

            false
        }

        let mut items = vec![];
        let mut create_ids: HashSet<_> = next.iter().map(|to| to.id).collect();

        let next_map =
            HashMap::<&str, &'a Index>::from_iter(next.iter().map(|to| (to.name.as_str(), to)));

        for previous in previous {
            let next = if let Some(next_id) = cx.rename_hints().get_index(previous.id) {
                cx.next().index(next_id)
            } else if let Some(next) = next_map.get(previous.name.as_str()) {
                next
            } else {
                items.push(IndicesDiffItem::DropIndex(previous));
                continue;
            };

            create_ids.remove(&next.id);

            if has_diff(cx, previous, next) {
                items.push(IndicesDiffItem::AlterIndex { previous, next });
            }
        }

        for index_id in create_ids {
            items.push(IndicesDiffItem::CreateIndex(cx.next().index(index_id)));
        }

        Self { items }
    }

    pub const fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl<'a> Deref for IndicesDiff<'a> {
    type Target = Vec<IndicesDiffItem<'a>>;

    fn deref(&self) -> &Self::Target {
        &self.items
    }
}

pub enum IndicesDiffItem<'a> {
    CreateIndex(&'a Index),
    DropIndex(&'a Index),
    AlterIndex {
        previous: &'a Index,
        next: &'a Index,
    },
}

#[cfg(test)]
mod tests {
    use crate::schema::db::{
        Column, ColumnId, DiffContext, Index, IndexColumn, IndexId, IndexOp, IndexScope,
        IndicesDiff, IndicesDiffItem, PrimaryKey, RenameHints, Schema, Table, TableId, Type,
    };
    use crate::stmt;

    fn make_column(table_id: usize, index: usize, name: &str) -> Column {
        Column {
            id: ColumnId {
                table: TableId(table_id),
                index,
            },
            name: name.to_string(),
            ty: stmt::Type::String,
            storage_ty: Type::Text,
            nullable: false,
            primary_key: false,
            auto_increment: false,
        }
    }

    fn make_index(
        table_id: usize,
        index: usize,
        name: &str,
        columns: Vec<(usize, IndexOp, IndexScope)>,
        unique: bool,
    ) -> Index {
        Index {
            id: IndexId {
                table: TableId(table_id),
                index,
            },
            name: name.to_string(),
            on: TableId(table_id),
            columns: columns
                .into_iter()
                .map(|(col_idx, op, scope)| IndexColumn {
                    column: ColumnId {
                        table: TableId(table_id),
                        index: col_idx,
                    },
                    op,
                    scope,
                })
                .collect(),
            unique,
            primary_key: false,
        }
    }

    fn make_schema_with_indices(
        table_id: usize,
        columns: Vec<Column>,
        indices: Vec<Index>,
    ) -> Schema {
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
            indices,
        });
        schema
    }

    #[test]
    fn test_no_diff_same_indices() {
        let columns = vec![make_column(0, 0, "id"), make_column(0, 1, "name")];

        let from_indices = vec![make_index(
            0,
            0,
            "idx_name",
            vec![(1, IndexOp::Eq, IndexScope::Local)],
            false,
        )];
        let to_indices = vec![make_index(
            0,
            0,
            "idx_name",
            vec![(1, IndexOp::Eq, IndexScope::Local)],
            false,
        )];

        let from_schema = make_schema_with_indices(0, columns.clone(), from_indices.clone());
        let to_schema = make_schema_with_indices(0, columns, to_indices.clone());
        let hints = RenameHints::new();
        let cx = DiffContext::new(&from_schema, &to_schema, &hints);

        let diff = IndicesDiff::from(&cx, &from_indices, &to_indices);
        assert!(diff.is_empty());
    }

    #[test]
    fn test_create_index() {
        let columns = vec![make_column(0, 0, "id"), make_column(0, 1, "name")];

        let from_indices = vec![];
        let to_indices = vec![make_index(
            0,
            0,
            "idx_name",
            vec![(1, IndexOp::Eq, IndexScope::Local)],
            false,
        )];

        let from_schema = make_schema_with_indices(0, columns.clone(), from_indices.clone());
        let to_schema = make_schema_with_indices(0, columns, to_indices.clone());
        let hints = RenameHints::new();
        let cx = DiffContext::new(&from_schema, &to_schema, &hints);

        let diff = IndicesDiff::from(&cx, &from_indices, &to_indices);
        assert_eq!(diff.items.len(), 1);
        assert!(matches!(diff.items[0], IndicesDiffItem::CreateIndex(_)));
        if let IndicesDiffItem::CreateIndex(idx) = diff.items[0] {
            assert_eq!(idx.name, "idx_name");
        }
    }

    #[test]
    fn test_drop_index() {
        let columns = vec![make_column(0, 0, "id"), make_column(0, 1, "name")];

        let from_indices = vec![make_index(
            0,
            0,
            "idx_name",
            vec![(1, IndexOp::Eq, IndexScope::Local)],
            false,
        )];
        let to_indices = vec![];

        let from_schema = make_schema_with_indices(0, columns.clone(), from_indices.clone());
        let to_schema = make_schema_with_indices(0, columns, to_indices.clone());
        let hints = RenameHints::new();
        let cx = DiffContext::new(&from_schema, &to_schema, &hints);

        let diff = IndicesDiff::from(&cx, &from_indices, &to_indices);
        assert_eq!(diff.items.len(), 1);
        assert!(matches!(diff.items[0], IndicesDiffItem::DropIndex(_)));
        if let IndicesDiffItem::DropIndex(idx) = diff.items[0] {
            assert_eq!(idx.name, "idx_name");
        }
    }

    #[test]
    fn test_alter_index_unique() {
        let columns = vec![make_column(0, 0, "id"), make_column(0, 1, "name")];

        let from_indices = vec![make_index(
            0,
            0,
            "idx_name",
            vec![(1, IndexOp::Eq, IndexScope::Local)],
            false,
        )];
        let to_indices = vec![make_index(
            0,
            0,
            "idx_name",
            vec![(1, IndexOp::Eq, IndexScope::Local)],
            true, // changed to unique
        )];

        let from_schema = make_schema_with_indices(0, columns.clone(), from_indices.clone());
        let to_schema = make_schema_with_indices(0, columns, to_indices.clone());
        let hints = RenameHints::new();
        let cx = DiffContext::new(&from_schema, &to_schema, &hints);

        let diff = IndicesDiff::from(&cx, &from_indices, &to_indices);
        assert_eq!(diff.items.len(), 1);
        assert!(matches!(diff.items[0], IndicesDiffItem::AlterIndex { .. }));
    }

    #[test]
    fn test_alter_index_columns() {
        let columns = vec![
            make_column(0, 0, "id"),
            make_column(0, 1, "name"),
            make_column(0, 2, "email"),
        ];

        let from_indices = vec![make_index(
            0,
            0,
            "idx_name",
            vec![(1, IndexOp::Eq, IndexScope::Local)],
            false,
        )];
        let to_indices = vec![make_index(
            0,
            0,
            "idx_name",
            vec![
                (1, IndexOp::Eq, IndexScope::Local),
                (2, IndexOp::Eq, IndexScope::Local),
            ],
            false,
        )];

        let from_schema = make_schema_with_indices(0, columns.clone(), from_indices.clone());
        let to_schema = make_schema_with_indices(0, columns, to_indices.clone());
        let hints = RenameHints::new();
        let cx = DiffContext::new(&from_schema, &to_schema, &hints);

        let diff = IndicesDiff::from(&cx, &from_indices, &to_indices);
        assert_eq!(diff.items.len(), 1);
        assert!(matches!(diff.items[0], IndicesDiffItem::AlterIndex { .. }));
    }

    #[test]
    fn test_alter_index_op() {
        let columns = vec![make_column(0, 0, "id"), make_column(0, 1, "name")];

        let from_indices = vec![make_index(
            0,
            0,
            "idx_name",
            vec![(1, IndexOp::Eq, IndexScope::Local)],
            false,
        )];
        let to_indices = vec![make_index(
            0,
            0,
            "idx_name",
            vec![(1, IndexOp::Sort(stmt::Direction::Asc), IndexScope::Local)],
            false,
        )];

        let from_schema = make_schema_with_indices(0, columns.clone(), from_indices.clone());
        let to_schema = make_schema_with_indices(0, columns, to_indices.clone());
        let hints = RenameHints::new();
        let cx = DiffContext::new(&from_schema, &to_schema, &hints);

        let diff = IndicesDiff::from(&cx, &from_indices, &to_indices);
        assert_eq!(diff.items.len(), 1);
        assert!(matches!(diff.items[0], IndicesDiffItem::AlterIndex { .. }));
    }

    #[test]
    fn test_alter_index_scope() {
        let columns = vec![make_column(0, 0, "id"), make_column(0, 1, "name")];

        let from_indices = vec![make_index(
            0,
            0,
            "idx_name",
            vec![(1, IndexOp::Eq, IndexScope::Local)],
            false,
        )];
        let to_indices = vec![make_index(
            0,
            0,
            "idx_name",
            vec![(1, IndexOp::Eq, IndexScope::Partition)],
            false,
        )];

        let from_schema = make_schema_with_indices(0, columns.clone(), from_indices.clone());
        let to_schema = make_schema_with_indices(0, columns, to_indices.clone());
        let hints = RenameHints::new();
        let cx = DiffContext::new(&from_schema, &to_schema, &hints);

        let diff = IndicesDiff::from(&cx, &from_indices, &to_indices);
        assert_eq!(diff.items.len(), 1);
        assert!(matches!(diff.items[0], IndicesDiffItem::AlterIndex { .. }));
    }

    #[test]
    fn test_rename_index_with_hint() {
        let columns = vec![make_column(0, 0, "id"), make_column(0, 1, "name")];

        let from_indices = vec![make_index(
            0,
            0,
            "old_idx_name",
            vec![(1, IndexOp::Eq, IndexScope::Local)],
            false,
        )];
        let to_indices = vec![make_index(
            0,
            0,
            "new_idx_name",
            vec![(1, IndexOp::Eq, IndexScope::Local)],
            false,
        )];

        let from_schema = make_schema_with_indices(0, columns.clone(), from_indices.clone());
        let to_schema = make_schema_with_indices(0, columns, to_indices.clone());

        let mut hints = RenameHints::new();
        hints.add_index_hint(
            IndexId {
                table: TableId(0),
                index: 0,
            },
            IndexId {
                table: TableId(0),
                index: 0,
            },
        );
        let cx = DiffContext::new(&from_schema, &to_schema, &hints);

        let diff = IndicesDiff::from(&cx, &from_indices, &to_indices);
        assert_eq!(diff.items.len(), 1);
        assert!(matches!(diff.items[0], IndicesDiffItem::AlterIndex { .. }));
        if let IndicesDiffItem::AlterIndex { previous, next } = diff.items[0] {
            assert_eq!(previous.name, "old_idx_name");
            assert_eq!(next.name, "new_idx_name");
        }
    }

    #[test]
    fn test_rename_index_without_hint_is_drop_and_create() {
        let columns = vec![make_column(0, 0, "id"), make_column(0, 1, "name")];

        let from_indices = vec![make_index(
            0,
            0,
            "old_idx_name",
            vec![(1, IndexOp::Eq, IndexScope::Local)],
            false,
        )];
        let to_indices = vec![make_index(
            0,
            0,
            "new_idx_name",
            vec![(1, IndexOp::Eq, IndexScope::Local)],
            false,
        )];

        let from_schema = make_schema_with_indices(0, columns.clone(), from_indices.clone());
        let to_schema = make_schema_with_indices(0, columns, to_indices.clone());
        let hints = RenameHints::new();
        let cx = DiffContext::new(&from_schema, &to_schema, &hints);

        let diff = IndicesDiff::from(&cx, &from_indices, &to_indices);
        assert_eq!(diff.items.len(), 2);

        let has_drop = diff
            .items
            .iter()
            .any(|item| matches!(item, IndicesDiffItem::DropIndex(_)));
        let has_create = diff
            .items
            .iter()
            .any(|item| matches!(item, IndicesDiffItem::CreateIndex(_)));
        assert!(has_drop);
        assert!(has_create);
    }

    #[test]
    fn test_index_with_renamed_column() {
        let from_columns = vec![make_column(0, 0, "id"), make_column(0, 1, "old_name")];
        let to_columns = vec![make_column(0, 0, "id"), make_column(0, 1, "new_name")];

        let from_indices = vec![make_index(
            0,
            0,
            "idx_name",
            vec![(1, IndexOp::Eq, IndexScope::Local)],
            false,
        )];
        let to_indices = vec![make_index(
            0,
            0,
            "idx_name",
            vec![(1, IndexOp::Eq, IndexScope::Local)],
            false,
        )];

        let from_schema = make_schema_with_indices(0, from_columns, from_indices.clone());
        let to_schema = make_schema_with_indices(0, to_columns, to_indices.clone());

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

        let diff = IndicesDiff::from(&cx, &from_indices, &to_indices);
        // Index should remain unchanged when column is renamed with hint
        assert!(diff.is_empty());
    }

    #[test]
    fn test_multiple_operations() {
        let columns = vec![
            make_column(0, 0, "id"),
            make_column(0, 1, "name"),
            make_column(0, 2, "email"),
        ];

        let from_indices = vec![
            make_index(
                0,
                0,
                "idx_name",
                vec![(1, IndexOp::Eq, IndexScope::Local)],
                false,
            ),
            make_index(
                0,
                1,
                "old_idx",
                vec![(2, IndexOp::Eq, IndexScope::Local)],
                false,
            ),
            make_index(
                0,
                2,
                "idx_to_drop",
                vec![(0, IndexOp::Eq, IndexScope::Local)],
                false,
            ),
        ];
        let to_indices = vec![
            make_index(
                0,
                0,
                "idx_name",
                vec![(1, IndexOp::Eq, IndexScope::Local)],
                true, // changed to unique
            ),
            make_index(
                0,
                1,
                "new_idx",
                vec![(2, IndexOp::Eq, IndexScope::Local)],
                false,
            ),
            make_index(
                0,
                2,
                "idx_added",
                vec![(1, IndexOp::Sort(stmt::Direction::Asc), IndexScope::Local)],
                false,
            ),
        ];

        let from_schema = make_schema_with_indices(0, columns.clone(), from_indices.clone());
        let to_schema = make_schema_with_indices(0, columns, to_indices.clone());

        let mut hints = RenameHints::new();
        hints.add_index_hint(
            IndexId {
                table: TableId(0),
                index: 1,
            },
            IndexId {
                table: TableId(0),
                index: 1,
            },
        );
        let cx = DiffContext::new(&from_schema, &to_schema, &hints);

        let diff = IndicesDiff::from(&cx, &from_indices, &to_indices);
        // Should have: 1 alter (idx_name unique changed), 1 alter (renamed), 1 drop (idx_to_drop), 1 create (idx_added)
        assert_eq!(diff.items.len(), 4);
    }
}

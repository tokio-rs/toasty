use super::Context;
use crate::schema::db::Index;

use hashbrown::{HashMap, HashSet};
use std::ops::Deref;

/// The set of differences between two index lists.
///
/// Computed by [`Indices::from`] and dereferences to `Vec<IndicesItem>` for
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
/// let d = diff::Indices::from(&cx, &[], &[]);
/// assert!(d.is_empty());
/// ```
pub struct Indices<'a> {
    items: Vec<IndicesItem<'a>>,
}

impl<'a> Indices<'a> {
    /// Computes the diff between two index slices.
    ///
    /// Uses [`Context`] to resolve rename hints for both indices and columns.
    /// Indices matched by name (or by rename hint) are compared; unmatched
    /// indices in `previous` become drops, and unmatched indices in `next`
    /// become creates.
    pub fn from(cx: &Context<'a>, previous: &'a [Index], next: &'a [Index]) -> Self {
        fn has_diff(cx: &Context<'_>, previous: &Index, next: &Index) -> bool {
            if previous.name != next.name
                || previous.columns.len() != next.columns.len()
                || previous.unique != next.unique
                || previous.primary_key != next.primary_key
            {
                return true;
            }

            for (previous_col, next_col) in previous.columns.iter().zip(next.columns.iter()) {
                if previous_col.op != next_col.op || previous_col.scope != next_col.scope {
                    return true;
                }

                let columns_match =
                    if let Some(renamed_to) = cx.rename_hints().get_column(previous_col.column) {
                        renamed_to == next_col.column
                    } else {
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
                items.push(IndicesItem::DropIndex(previous));
                continue;
            };

            create_ids.remove(&next.id);

            if has_diff(cx, previous, next) {
                items.push(IndicesItem::AlterIndex { previous, next });
            }
        }

        for index_id in create_ids {
            items.push(IndicesItem::CreateIndex(cx.next().index(index_id)));
        }

        Self { items }
    }

    /// Returns `true` if there are no index changes.
    pub const fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl<'a> Deref for Indices<'a> {
    type Target = Vec<IndicesItem<'a>>;

    fn deref(&self) -> &Self::Target {
        &self.items
    }
}

/// A single change detected between two index lists.
pub enum IndicesItem<'a> {
    /// A new index was created.
    CreateIndex(&'a Index),
    /// An existing index was dropped.
    DropIndex(&'a Index),
    /// An index was modified (name, columns, uniqueness, or other property changed).
    AlterIndex {
        /// The index definition before the change.
        previous: &'a Index,
        /// The index definition after the change.
        next: &'a Index,
    },
}

#[cfg(test)]
mod tests {
    use crate::schema::db::{
        Column, ColumnId, Index, IndexColumn, IndexId, IndexOp, IndexScope, PrimaryKey, Schema,
        Table, TableId, Type,
        diff::{self, Indices, IndicesItem},
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
            versionable: false,
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
        let hints = diff::RenameHints::new();
        let cx = diff::Context::new(&from_schema, &to_schema, &hints);

        let d = Indices::from(&cx, &from_indices, &to_indices);
        assert!(d.is_empty());
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
        let hints = diff::RenameHints::new();
        let cx = diff::Context::new(&from_schema, &to_schema, &hints);

        let d = Indices::from(&cx, &from_indices, &to_indices);
        assert_eq!(d.len(), 1);
        assert!(matches!(d[0], IndicesItem::CreateIndex(_)));
        if let IndicesItem::CreateIndex(idx) = d[0] {
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
        let hints = diff::RenameHints::new();
        let cx = diff::Context::new(&from_schema, &to_schema, &hints);

        let d = Indices::from(&cx, &from_indices, &to_indices);
        assert_eq!(d.len(), 1);
        assert!(matches!(d[0], IndicesItem::DropIndex(_)));
        if let IndicesItem::DropIndex(idx) = d[0] {
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
            true,
        )];

        let from_schema = make_schema_with_indices(0, columns.clone(), from_indices.clone());
        let to_schema = make_schema_with_indices(0, columns, to_indices.clone());
        let hints = diff::RenameHints::new();
        let cx = diff::Context::new(&from_schema, &to_schema, &hints);

        let d = Indices::from(&cx, &from_indices, &to_indices);
        assert_eq!(d.len(), 1);
        assert!(matches!(d[0], IndicesItem::AlterIndex { .. }));
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
        let hints = diff::RenameHints::new();
        let cx = diff::Context::new(&from_schema, &to_schema, &hints);

        let d = Indices::from(&cx, &from_indices, &to_indices);
        assert_eq!(d.len(), 1);
        assert!(matches!(d[0], IndicesItem::AlterIndex { .. }));
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
        let hints = diff::RenameHints::new();
        let cx = diff::Context::new(&from_schema, &to_schema, &hints);

        let d = Indices::from(&cx, &from_indices, &to_indices);
        assert_eq!(d.len(), 1);
        assert!(matches!(d[0], IndicesItem::AlterIndex { .. }));
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
        let hints = diff::RenameHints::new();
        let cx = diff::Context::new(&from_schema, &to_schema, &hints);

        let d = Indices::from(&cx, &from_indices, &to_indices);
        assert_eq!(d.len(), 1);
        assert!(matches!(d[0], IndicesItem::AlterIndex { .. }));
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

        let mut hints = diff::RenameHints::new();
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
        let cx = diff::Context::new(&from_schema, &to_schema, &hints);

        let d = Indices::from(&cx, &from_indices, &to_indices);
        assert_eq!(d.len(), 1);
        assert!(matches!(d[0], IndicesItem::AlterIndex { .. }));
        if let IndicesItem::AlterIndex { previous, next } = d[0] {
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
        let hints = diff::RenameHints::new();
        let cx = diff::Context::new(&from_schema, &to_schema, &hints);

        let d = Indices::from(&cx, &from_indices, &to_indices);
        assert_eq!(d.len(), 2);

        let has_drop = d
            .iter()
            .any(|item| matches!(item, IndicesItem::DropIndex(_)));
        let has_create = d
            .iter()
            .any(|item| matches!(item, IndicesItem::CreateIndex(_)));
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

        let d = Indices::from(&cx, &from_indices, &to_indices);
        assert!(d.is_empty());
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
                true,
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

        let mut hints = diff::RenameHints::new();
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
        let cx = diff::Context::new(&from_schema, &to_schema, &hints);

        let d = Indices::from(&cx, &from_indices, &to_indices);
        assert_eq!(d.len(), 4);
    }
}

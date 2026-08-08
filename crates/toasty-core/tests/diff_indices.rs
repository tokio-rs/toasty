use toasty_core::schema::{
    db::{
        Column, ColumnId, Index, IndexColumn, IndexId, IndexOp, IndexScope, PrimaryKey, Schema,
        Table, TableId, Type,
    },
    diff,
};
use toasty_core::stmt;

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

fn make_schema_with_indices(table_id: usize, columns: Vec<Column>, indices: Vec<Index>) -> Schema {
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
fn no_diff_same_indices() {
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

    let d = diff::Index::diff(&cx, &from_indices, &to_indices);
    assert!(d.is_empty());
}

#[test]
fn create_index() {
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

    let d = diff::Index::diff(&cx, &from_indices, &to_indices);
    assert_eq!(d.len(), 1);
    assert!(matches!(d[0], diff::Index::Create(_)));
    if let diff::Index::Create(idx) = d[0] {
        assert_eq!(idx.name, "idx_name");
    }
}

#[test]
fn drop_index() {
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

    let d = diff::Index::diff(&cx, &from_indices, &to_indices);
    assert_eq!(d.len(), 1);
    assert!(matches!(d[0], diff::Index::Drop(_)));
    if let diff::Index::Drop(idx) = d[0] {
        assert_eq!(idx.name, "idx_name");
    }
}

#[test]
fn alter_index_unique() {
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

    let d = diff::Index::diff(&cx, &from_indices, &to_indices);
    assert_eq!(d.len(), 1);
    assert!(matches!(d[0], diff::Index::Alter { .. }));
}

#[test]
fn alter_index_columns() {
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

    let d = diff::Index::diff(&cx, &from_indices, &to_indices);
    assert_eq!(d.len(), 1);
    assert!(matches!(d[0], diff::Index::Alter { .. }));
}

#[test]
fn alter_index_op() {
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

    let d = diff::Index::diff(&cx, &from_indices, &to_indices);
    assert_eq!(d.len(), 1);
    assert!(matches!(d[0], diff::Index::Alter { .. }));
}

#[test]
fn alter_index_scope() {
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

    let d = diff::Index::diff(&cx, &from_indices, &to_indices);
    assert_eq!(d.len(), 1);
    assert!(matches!(d[0], diff::Index::Alter { .. }));
}

#[test]
fn rename_index_with_hint() {
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

    let d = diff::Index::diff(&cx, &from_indices, &to_indices);
    assert_eq!(d.len(), 1);
    assert!(matches!(d[0], diff::Index::Alter { .. }));
    if let diff::Index::Alter { previous, next } = d[0] {
        assert_eq!(previous.name, "old_idx_name");
        assert_eq!(next.name, "new_idx_name");
    }
}

#[test]
fn rename_index_without_hint_is_drop_and_create() {
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

    let d = diff::Index::diff(&cx, &from_indices, &to_indices);
    assert_eq!(d.len(), 2);

    let has_drop = d.iter().any(|item| matches!(item, diff::Index::Drop(_)));
    let has_create = d.iter().any(|item| matches!(item, diff::Index::Create(_)));
    assert!(has_drop);
    assert!(has_create);
}

#[test]
fn index_with_renamed_column() {
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

    let d = diff::Index::diff(&cx, &from_indices, &to_indices);
    assert!(d.is_empty());
}

#[test]
fn multiple_operations() {
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

    let d = diff::Index::diff(&cx, &from_indices, &to_indices);
    assert_eq!(d.len(), 4);
}

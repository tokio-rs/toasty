use toasty_core::schema::{
    db::{Column, ColumnId, IndexId, PrimaryKey, Schema, Table, TableId, Type},
    diff,
};
use toasty_core::stmt;

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
fn no_diff_same_columns() {
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

    let d = diff::Column::diff(&cx, &from_cols, &to_cols);
    assert!(d.is_empty());
}

#[test]
fn add_column() {
    let from_cols = vec![make_column(0, 0, "id", Type::Integer(8), false)];
    let to_cols = vec![
        make_column(0, 0, "id", Type::Integer(8), false),
        make_column(0, 1, "name", Type::Text, false),
    ];

    let from_schema = make_schema_with_columns(0, from_cols.clone());
    let to_schema = make_schema_with_columns(0, to_cols.clone());
    let hints = diff::RenameHints::new();
    let cx = diff::Context::new(&from_schema, &to_schema, &hints);

    let d = diff::Column::diff(&cx, &from_cols, &to_cols);
    assert_eq!(d.len(), 1);
    assert!(matches!(d[0], diff::Column::Add(_)));
    if let diff::Column::Add(col) = d[0] {
        assert_eq!(col.name, "name");
    }
}

#[test]
fn drop_column() {
    let from_cols = vec![
        make_column(0, 0, "id", Type::Integer(8), false),
        make_column(0, 1, "name", Type::Text, false),
    ];
    let to_cols = vec![make_column(0, 0, "id", Type::Integer(8), false)];

    let from_schema = make_schema_with_columns(0, from_cols.clone());
    let to_schema = make_schema_with_columns(0, to_cols.clone());
    let hints = diff::RenameHints::new();
    let cx = diff::Context::new(&from_schema, &to_schema, &hints);

    let d = diff::Column::diff(&cx, &from_cols, &to_cols);
    assert_eq!(d.len(), 1);
    assert!(matches!(d[0], diff::Column::Drop(_)));
    if let diff::Column::Drop(col) = d[0] {
        assert_eq!(col.name, "name");
    }
}

#[test]
fn alter_column_type() {
    let from_cols = vec![make_column(0, 0, "id", Type::Integer(8), false)];
    let to_cols = vec![make_column(0, 0, "id", Type::Text, false)];

    let from_schema = make_schema_with_columns(0, from_cols.clone());
    let to_schema = make_schema_with_columns(0, to_cols.clone());
    let hints = diff::RenameHints::new();
    let cx = diff::Context::new(&from_schema, &to_schema, &hints);

    let d = diff::Column::diff(&cx, &from_cols, &to_cols);
    assert_eq!(d.len(), 1);
    assert!(matches!(d[0], diff::Column::Alter { .. }));
}

#[test]
fn alter_column_nullable() {
    let from_cols = vec![make_column(0, 0, "id", Type::Integer(8), false)];
    let to_cols = vec![make_column(0, 0, "id", Type::Integer(8), true)];

    let from_schema = make_schema_with_columns(0, from_cols.clone());
    let to_schema = make_schema_with_columns(0, to_cols.clone());
    let hints = diff::RenameHints::new();
    let cx = diff::Context::new(&from_schema, &to_schema, &hints);

    let d = diff::Column::diff(&cx, &from_cols, &to_cols);
    assert_eq!(d.len(), 1);
    assert!(matches!(d[0], diff::Column::Alter { .. }));
}

#[test]
fn rename_column_with_hint() {
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

    let d = diff::Column::diff(&cx, &from_cols, &to_cols);
    assert_eq!(d.len(), 1);
    assert!(matches!(d[0], diff::Column::Alter { .. }));
    if let diff::Column::Alter { previous, next } = d[0] {
        assert_eq!(previous.name, "old_name");
        assert_eq!(next.name, "new_name");
    }
}

#[test]
fn rename_column_without_hint_is_drop_and_add() {
    let from_cols = vec![make_column(0, 0, "old_name", Type::Text, false)];
    let to_cols = vec![make_column(0, 0, "new_name", Type::Text, false)];

    let from_schema = make_schema_with_columns(0, from_cols.clone());
    let to_schema = make_schema_with_columns(0, to_cols.clone());
    let hints = diff::RenameHints::new();
    let cx = diff::Context::new(&from_schema, &to_schema, &hints);

    let d = diff::Column::diff(&cx, &from_cols, &to_cols);
    assert_eq!(d.len(), 2);

    let has_drop = d.iter().any(|item| matches!(item, diff::Column::Drop(_)));
    let has_add = d.iter().any(|item| matches!(item, diff::Column::Add(_)));
    assert!(has_drop);
    assert!(has_add);
}

#[test]
fn multiple_operations() {
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

    let d = diff::Column::diff(&cx, &from_cols, &to_cols);
    assert_eq!(d.len(), 4);
}

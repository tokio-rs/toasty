use toasty_core::schema::{
    db::{Column, ColumnId, IndexId, PrimaryKey, Schema, Table, TableId, Type},
    diff,
};
use toasty_core::stmt;

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
fn no_diff_same_tables() {
    let from_tables = vec![make_table(0, "users", 2), make_table(1, "posts", 3)];
    let to_tables = vec![make_table(0, "users", 2), make_table(1, "posts", 3)];

    let from_schema = make_schema(from_tables.clone());
    let to_schema = make_schema(to_tables.clone());
    let hints = diff::RenameHints::new();
    let cx = diff::Context::new(&from_schema, &to_schema, &hints);

    let d = diff::Table::diff(&cx, &from_tables, &to_tables);
    assert_eq!(d.len(), 0);
}

#[test]
fn create_table() {
    let from_tables = vec![make_table(0, "users", 2)];
    let to_tables = vec![make_table(0, "users", 2), make_table(1, "posts", 3)];

    let from_schema = make_schema(from_tables.clone());
    let to_schema = make_schema(to_tables.clone());
    let hints = diff::RenameHints::new();
    let cx = diff::Context::new(&from_schema, &to_schema, &hints);

    let d = diff::Table::diff(&cx, &from_tables, &to_tables);
    assert_eq!(d.len(), 1);
    assert!(matches!(d[0], diff::Table::Create(_)));
    if let diff::Table::Create(table) = d[0] {
        assert_eq!(table.name, "posts");
    }
}

#[test]
fn drop_table() {
    let from_tables = vec![make_table(0, "users", 2), make_table(1, "posts", 3)];
    let to_tables = vec![make_table(0, "users", 2)];

    let from_schema = make_schema(from_tables.clone());
    let to_schema = make_schema(to_tables.clone());
    let hints = diff::RenameHints::new();
    let cx = diff::Context::new(&from_schema, &to_schema, &hints);

    let d = diff::Table::diff(&cx, &from_tables, &to_tables);
    assert_eq!(d.len(), 1);
    assert!(matches!(d[0], diff::Table::Drop(_)));
    if let diff::Table::Drop(table) = d[0] {
        assert_eq!(table.name, "posts");
    }
}

#[test]
fn rename_table_with_hint() {
    let from_tables = vec![make_table(0, "old_users", 2)];
    let to_tables = vec![make_table(0, "new_users", 2)];

    let from_schema = make_schema(from_tables.clone());
    let to_schema = make_schema(to_tables.clone());

    let mut hints = diff::RenameHints::new();
    hints.add_table_hint(TableId(0), TableId(0));
    let cx = diff::Context::new(&from_schema, &to_schema, &hints);

    let d = diff::Table::diff(&cx, &from_tables, &to_tables);
    assert_eq!(d.len(), 1);
    assert!(matches!(d[0], diff::Table::Alter { .. }));
    if let diff::Table::Alter { previous, next, .. } = &d[0] {
        assert_eq!(previous.name, "old_users");
        assert_eq!(next.name, "new_users");
    }
}

#[test]
fn rename_table_without_hint_is_drop_and_create() {
    let from_tables = vec![make_table(0, "old_users", 2)];
    let to_tables = vec![make_table(0, "new_users", 2)];

    let from_schema = make_schema(from_tables.clone());
    let to_schema = make_schema(to_tables.clone());
    let hints = diff::RenameHints::new();
    let cx = diff::Context::new(&from_schema, &to_schema, &hints);

    let d = diff::Table::diff(&cx, &from_tables, &to_tables);
    assert_eq!(d.len(), 2);

    let has_drop = d.iter().any(|item| matches!(item, diff::Table::Drop(_)));
    let has_create = d.iter().any(|item| matches!(item, diff::Table::Create(_)));
    assert!(has_drop);
    assert!(has_create);
}

#[test]
fn alter_table_column_change() {
    let from_tables = vec![make_table(0, "users", 2)];
    let to_tables = vec![make_table(0, "users", 3)];

    let from_schema = make_schema(from_tables.clone());
    let to_schema = make_schema(to_tables.clone());
    let hints = diff::RenameHints::new();
    let cx = diff::Context::new(&from_schema, &to_schema, &hints);

    let d = diff::Table::diff(&cx, &from_tables, &to_tables);
    assert_eq!(d.len(), 1);
    assert!(matches!(d[0], diff::Table::Alter { .. }));
}

#[test]
fn multiple_operations() {
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

    let d = diff::Table::diff(&cx, &from_tables, &to_tables);
    assert_eq!(d.len(), 4);
}

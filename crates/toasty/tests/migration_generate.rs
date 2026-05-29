#![cfg(feature = "migration")]

use toasty::migration;
use toasty::schema::{db, diff};
use toasty_core::stmt;
use toasty_driver_sqlite::Sqlite;

fn users_schema() -> db::Schema {
    let table_id = db::TableId(0);
    let id = db::ColumnId {
        table: table_id,
        index: 0,
    };

    db::Schema {
        tables: vec![db::Table {
            id: table_id,
            name: "users".to_string(),
            columns: vec![db::Column {
                id,
                name: "id".to_string(),
                ty: stmt::Type::I64,
                storage_ty: db::Type::Integer(8),
                nullable: false,
                primary_key: true,
                auto_increment: false,
                versionable: false,
            }],
            primary_key: db::PrimaryKey {
                columns: vec![id],
                index: db::IndexId {
                    table: table_id,
                    index: 0,
                },
            },
            indices: vec![],
        }],
    }
}

#[test]
fn generate_returns_none_for_empty_diff() {
    let schema = users_schema();
    let hints = diff::RenameHints::new();
    let driver = Sqlite::in_memory();

    let generated = migration::generate(&driver, &schema, &schema, &hints);

    assert!(generated.is_none());
}

#[test]
fn generate_returns_migration_and_next_snapshot() {
    let previous = db::Schema::default();
    let next = users_schema();
    let hints = diff::RenameHints::new();
    let driver = Sqlite::in_memory();

    let generated = migration::generate(&driver, &previous, &next, &hints).unwrap();

    assert_eq!(generated.snapshot.schema.tables[0].name, "users");
    let db::Migration::Sql(sql) = generated.migration;
    assert!(sql.contains("CREATE TABLE"));
    assert!(sql.contains("users"));
}

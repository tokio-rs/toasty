#![cfg(feature = "migration")]

use toasty::migration;
use toasty::schema::{db, diff};
use toasty_core::stmt;
use toasty_driver_sqlite::Sqlite;

fn round_trip_snapshot(schema: db::Schema) -> (String, migration::Snapshot) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("snapshot.toml");

    migration::Snapshot::new(schema).save(&path).unwrap();

    let contents = std::fs::read_to_string(&path).unwrap();
    let snapshot = migration::Snapshot::load(path).unwrap();
    (contents, snapshot)
}

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

#[test]
fn snapshot_round_trips_empty_indices() {
    let (_, snapshot) = round_trip_snapshot(users_schema());

    assert!(snapshot.schema.tables[0].indices.is_empty());
}

#[test]
fn snapshot_round_trips_unconstrained_numeric() {
    let mut schema = users_schema();
    schema.tables[0].columns[0].storage_ty = db::Type::Numeric(None);

    let (contents, snapshot) = round_trip_snapshot(schema);

    assert!(contents.contains("storage_ty = { Numeric = [] }"));
    assert_eq!(
        snapshot.schema.tables[0].columns[0].storage_ty,
        db::Type::Numeric(None)
    );
}

#[test]
fn snapshot_round_trips_bounded_numeric() {
    let mut schema = users_schema();
    schema.tables[0].columns[0].storage_ty = db::Type::Numeric(Some((28, 10)));

    let (contents, snapshot) = round_trip_snapshot(schema);

    assert!(contents.contains("storage_ty = { Numeric = [28, 10] }"));
    assert_eq!(
        snapshot.schema.tables[0].columns[0].storage_ty,
        db::Type::Numeric(Some((28, 10)))
    );
}

#[test]
fn snapshot_round_trips_list_of_unconstrained_numeric() {
    let mut schema = users_schema();
    schema.tables[0].columns[0].storage_ty = db::Type::List(Box::new(db::Type::Numeric(None)));

    let (contents, snapshot) = round_trip_snapshot(schema);

    assert!(contents.contains("storage_ty = { List = { Numeric = [] } }"));
    assert_eq!(
        snapshot.schema.tables[0].columns[0].storage_ty,
        db::Type::List(Box::new(db::Type::Numeric(None)))
    );
}

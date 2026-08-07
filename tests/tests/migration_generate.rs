use toasty::db::Capability;
use toasty_core::{schema::db, stmt};

#[test]
fn migration_generate_with_decimal_model_writes_snapshot() {
    #[derive(Debug, toasty::Model)]
    struct SomeModel {
        #[key]
        #[auto]
        id: u64,

        weight: rust_decimal::Decimal,
    }

    let mut builder = toasty::Db::builder();
    builder.models(toasty::models!(SomeModel));
    let app_schema = builder.build_app_schema().unwrap();

    let schema = toasty_core::schema::Builder::new()
        .table_name_prefix("svc_")
        .build(app_schema, &Capability::POSTGRESQL)
        .unwrap();

    let generated = toasty::migration::generate(
        &Capability::POSTGRESQL,
        &db::Schema::default(),
        &schema.db,
        &Default::default(),
    )
    .expect("schema differs from empty; expected a migration");

    let db::Migration::Sql(sql) = &generated.migration;
    assert!(sql.contains("CREATE TABLE \"svc_some_models\""), "{sql}");

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("0000_snapshot.toml");
    generated.snapshot.save(&path).unwrap();

    let snapshot = toasty::migration::Snapshot::load(&path).unwrap();
    let weight = snapshot.schema.tables[0]
        .columns
        .iter()
        .find(|column| column.name == "weight")
        .unwrap();
    assert_eq!(weight.ty, stmt::Type::Decimal);
    assert_eq!(weight.storage_ty, db::Type::Numeric(None));
}

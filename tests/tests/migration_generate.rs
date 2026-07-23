use async_trait::async_trait;
use std::{borrow::Cow, sync::Arc};
use toasty::db::{Capability, ConnectContext, Driver, ExecResponse};
use toasty_core::{
    Schema,
    driver::{Connection, Operation},
    schema::{
        db::{AppliedMigration, Migration, Type},
        diff,
    },
    stmt,
};

#[derive(Debug)]
struct PostgresSchemaDriver;

#[async_trait]
impl Driver for PostgresSchemaDriver {
    fn url(&self) -> Cow<'_, str> {
        "postgresql://test".into()
    }

    fn capability(&self) -> &'static Capability {
        &Capability::POSTGRESQL
    }

    async fn connect(&self, _cx: &ConnectContext) -> toasty::Result<Box<dyn Connection>> {
        Ok(Box::new(SchemaConnection))
    }

    fn generate_migration(&self, _schema_diff: &diff::Schema<'_>) -> Migration {
        Migration::Sql("-- generated migration".to_string())
    }

    async fn reset_db(&self) -> toasty::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
struct SchemaConnection;

#[async_trait]
impl Connection for SchemaConnection {
    async fn exec(
        &mut self,
        _schema: &Arc<Schema>,
        _plan: Operation,
    ) -> toasty::Result<ExecResponse> {
        unreachable!()
    }

    async fn push_schema(&mut self, _schema: &Schema) -> toasty::Result<()> {
        unreachable!()
    }

    async fn applied_migrations(&mut self) -> toasty::Result<Vec<AppliedMigration>> {
        unreachable!()
    }

    async fn apply_migration(
        &mut self,
        _id: u64,
        _name: &str,
        _migration: &Migration,
    ) -> toasty::Result<()> {
        unreachable!()
    }
}

#[tokio::test]
async fn migration_generate_with_decimal_model_writes_snapshot() {
    #[derive(Debug, toasty::Model)]
    struct SomeModel {
        #[key]
        #[auto]
        id: u64,

        weight: rust_decimal::Decimal,
    }

    let db = toasty::Db::builder()
        .models(toasty::models!(SomeModel))
        .table_name_prefix("svc_")
        .build(PostgresSchemaDriver)
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let config =
        toasty_cli::Config::new().migration(toasty_cli::MigrationConfig::new().path(dir.path()));

    toasty_cli::ToastyCli::with_config(db, config)
        .parse_from(["toasty", "migration", "generate"])
        .await
        .unwrap();

    assert!(dir.path().join("migrations/0000_migration.sql").is_file());

    let snapshot =
        toasty::migration::Snapshot::load(dir.path().join("snapshots/0000_snapshot.toml")).unwrap();
    let weight = snapshot.schema.tables[0]
        .columns
        .iter()
        .find(|column| column.name == "weight")
        .unwrap();
    assert_eq!(weight.ty, stmt::Type::Decimal);
    assert_eq!(weight.storage_ty, Type::Numeric(None));
}

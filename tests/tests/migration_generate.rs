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
    #[table(comment = "Weighted records")]
    struct SomeModel {
        #[key]
        #[auto]
        id: u64,

        #[column(comment = "Measured weight")]
        weight: rust_decimal::Decimal,
    }

    let db = toasty::Db::builder()
        .models(toasty::models!(SomeModel))
        .table_name_prefix("svc_")
        .build(PostgresSchemaDriver)
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let config = toasty_cli::Config::new().migration(
        toasty_cli::MigrationConfig::new()
            .path(dir.path())
            .schema_comments(true),
    );

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
    assert_eq!(
        snapshot.schema.tables[0].comment.as_deref(),
        Some("Weighted records")
    );
    assert_eq!(weight.comment.as_deref(), Some("Measured weight"));
}

#[tokio::test]
async fn disabled_schema_comments_carry_managed_comments_forward() {
    #[derive(Debug, toasty::Model)]
    #[table(comment = "Current declaration")]
    struct SomeModel {
        #[key]
        #[auto]
        id: u64,

        #[column(comment = "Current field declaration")]
        weight: rust_decimal::Decimal,
    }

    let db = toasty::Db::builder()
        .models(toasty::models!(SomeModel))
        .build(PostgresSchemaDriver)
        .await
        .unwrap();
    let mut previous = db.schema().db.clone();
    previous.tables[0].comment = Some("Managed table comment".to_string());
    previous.tables[0].columns[1].comment = Some("Managed column comment".to_string());

    let mut next = db.schema().db.clone();
    next.tables[0].columns[1].storage_ty = Type::Numeric(Some((28, 10)));

    let generated = toasty::migration::generate_with_options(
        db.driver(),
        &previous,
        &next,
        &diff::RenameHints::new(),
        toasty::migration::GenerateOptions::new(),
    )
    .unwrap();

    assert_eq!(
        generated.snapshot.schema.tables[0].comment.as_deref(),
        Some("Managed table comment")
    );
    assert_eq!(
        generated.snapshot.schema.tables[0].columns[1]
            .comment
            .as_deref(),
        Some("Managed column comment")
    );
}

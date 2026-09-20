use async_trait::async_trait;
use std::{borrow::Cow, fs, path::Path, process::Command, sync::Arc};
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
    let db = schema_db().await;
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

#[tokio::test]
async fn migration_generate_uses_toasty_toml() {
    if !in_project(Some("[migration]\nprefix_style = \"Timestamp\"\n")) {
        return;
    }

    toasty_cli::ToastyCli::new(schema_db().await)
        .parse_from(["toasty", "migration", "generate"])
        .await
        .unwrap();

    let history = toasty::migration::History::load("toasty/history.toml").unwrap();
    let [entry] = history.entries() else {
        panic!("expected one migration");
    };
    let prefix = entry.name.strip_suffix("_migration.sql").unwrap();
    jiff::civil::DateTime::strptime("%Y%m%d_%H%M%S", prefix).unwrap();
    assert!(Path::new("toasty/migrations").join(&entry.name).is_file());
    assert_eq!(entry.snapshot_name, format!("{prefix}_snapshot.toml"));
    assert!(
        Path::new("toasty/snapshots")
            .join(&entry.snapshot_name)
            .is_file()
    );
}

#[tokio::test]
async fn migration_generate_without_toasty_toml_uses_defaults() {
    if !in_project(None) {
        return;
    }

    toasty_cli::ToastyCli::new(schema_db().await)
        .parse_from(["toasty", "migration", "generate"])
        .await
        .unwrap();

    assert!(Path::new("toasty/migrations/0000_migration.sql").is_file());
}

#[tokio::test]
async fn migration_generate_rejects_invalid_toasty_toml() {
    if !in_project(Some("[migration]\nprefix_style = \"invalid\"\n")) {
        return;
    }

    let err = toasty_cli::ToastyCli::new(schema_db().await)
        .parse_from(["toasty", "migration", "generate"])
        .await
        .unwrap_err();

    let message = format!("{err:#}");
    assert!(message.contains("failed to parse Toasty config file at `Toasty.toml`"));
    assert!(message.contains("invalid"));
    assert!(!Path::new("toasty").exists());
}

#[tokio::test]
async fn migration_generate_with_config_ignores_toasty_toml() {
    if !in_project(Some("invalid TOML")) {
        return;
    }

    let config =
        toasty_cli::Config::new().migration(toasty_cli::MigrationConfig::new().path("custom"));
    toasty_cli::ToastyCli::with_config(schema_db().await, config)
        .parse_from(["toasty", "migration", "generate"])
        .await
        .unwrap();

    assert!(Path::new("custom/migrations/0000_migration.sql").is_file());
    assert!(!Path::new("toasty").exists());
}

#[test]
fn toasty_toml_defaults_for_omitted_settings() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Toasty.toml");

    for contents in ["", "[migration]\n"] {
        fs::write(&path, contents).unwrap();
        assert_eq!(
            toasty_cli::Config::load_from(&path).unwrap(),
            toasty_cli::Config::default()
        );
    }
}

fn in_project(config: Option<&str>) -> bool {
    let thread = std::thread::current();
    let test_name = thread.name().unwrap();
    if std::env::var("TOASTY_CLI_TEST_CHILD").as_deref() == Ok(test_name) {
        return true;
    }

    let dir = tempfile::tempdir().unwrap();
    if let Some(config) = config {
        fs::write(dir.path().join("Toasty.toml"), config).unwrap();
    }

    // Isolate the working directory from tests running in parallel.
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test_name, "--nocapture"])
        .env("TOASTY_CLI_TEST_CHILD", test_name)
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    false
}

async fn schema_db() -> toasty::Db {
    #[derive(Debug, toasty::Model)]
    struct SomeModel {
        #[key]
        #[auto]
        id: u64,

        weight: rust_decimal::Decimal,
    }

    toasty::Db::builder()
        .models(toasty::models!(SomeModel))
        .table_name_prefix("svc_")
        .build(PostgresSchemaDriver)
        .await
        .unwrap()
}

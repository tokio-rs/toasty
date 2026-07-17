#![cfg(feature = "migration")]

use std::{
    borrow::Cow,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use toasty::codegen_support::core::{
    Result, Schema,
    driver::{Capability, ConnectContext, Connection, Driver, ExecResponse, Operation},
    schema::{
        db::{AppliedMigration, Migration},
        diff,
    },
};

#[derive(Debug, toasty::Model)]
struct TestModel {
    #[key]
    id: u64,
}

#[derive(Debug, Default)]
struct MigrationState {
    applied: Mutex<Vec<u64>>,
    sql: Mutex<Vec<String>>,
}

#[derive(Debug)]
struct MigrationDriver {
    states: Arc<Mutex<Vec<Arc<MigrationState>>>>,
}

impl MigrationDriver {
    fn new() -> (Self, Arc<Mutex<Vec<Arc<MigrationState>>>>) {
        let states = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                states: states.clone(),
            },
            states,
        )
    }
}

#[async_trait]
impl Driver for MigrationDriver {
    fn url(&self) -> Cow<'_, str> {
        Cow::Borrowed("test:embedded-migrations")
    }

    fn capability(&self) -> &'static Capability {
        &Capability::SQLITE
    }

    async fn connect(&self, _cx: &ConnectContext) -> Result<Box<dyn Connection>> {
        let state = Arc::new(MigrationState::default());
        self.states.lock().unwrap().push(state.clone());
        Ok(Box::new(MigrationConnection { state }))
    }

    fn max_connections(&self) -> Option<usize> {
        Some(1)
    }

    fn generate_migration(&self, _schema_diff: &diff::Schema<'_>) -> Migration {
        Migration::new_sql(String::new())
    }

    async fn reset_db(&self) -> Result<()> {
        for state in self.states.lock().unwrap().iter() {
            state.applied.lock().unwrap().clear();
            state.sql.lock().unwrap().clear();
        }
        Ok(())
    }
}

#[derive(Debug)]
struct MigrationConnection {
    state: Arc<MigrationState>,
}

#[async_trait]
impl Connection for MigrationConnection {
    async fn exec(&mut self, _schema: &Arc<Schema>, _op: Operation) -> Result<ExecResponse> {
        unreachable!("migration test does not execute query plans")
    }

    async fn push_schema(&mut self, _schema: &Schema) -> Result<()> {
        Ok(())
    }

    async fn applied_migrations(&mut self) -> Result<Vec<AppliedMigration>> {
        Ok(self
            .state
            .applied
            .lock()
            .unwrap()
            .iter()
            .copied()
            .map(AppliedMigration::new)
            .collect())
    }

    async fn apply_migration(&mut self, id: u64, _name: &str, migration: &Migration) -> Result<()> {
        self.state.applied.lock().unwrap().push(id);
        self.state
            .sql
            .lock()
            .unwrap()
            .push(migration.statements().join("\n"));
        Ok(())
    }
}

async fn setup_db() -> (toasty::Db, Arc<MigrationState>) {
    let (driver, states) = MigrationDriver::new();
    let db = toasty::Db::builder()
        .models(toasty::models!(TestModel))
        .build(driver)
        .await
        .unwrap();
    let states = states.lock().unwrap();
    assert_eq!(states.len(), 1);
    let state = states[0].clone();
    (db, state)
}

#[tokio::test]
async fn embedded_migrations_apply_per_database_and_skip_applied_ids() {
    let (base_db, base_state) = setup_db().await;
    let (log_db, log_state) = setup_db().await;
    let base = toasty::embed_migrations!("tests/fixtures/embedded_migrations/base");
    let log = toasty::embed_migrations!("tests/fixtures/embedded_migrations/log");

    let base_report = base.apply(&base_db).await.unwrap();
    let log_report = log.apply(&log_db).await.unwrap();

    assert_eq!(base_report.applied(), 1);
    assert_eq!(base_report.skipped(), 0);
    assert_eq!(log_report.applied(), 1);
    assert_eq!(log_report.skipped(), 0);
    assert_eq!(*base_state.applied.lock().unwrap(), vec![101]);
    assert_eq!(*log_state.applied.lock().unwrap(), vec![202]);
    assert!(base_state.sql.lock().unwrap()[0].contains("base_embedded_items"));
    assert!(log_state.sql.lock().unwrap()[0].contains("log_embedded_items"));

    let repeated = base.apply(&base_db).await.unwrap();
    assert_eq!(repeated.applied(), 0);
    assert_eq!(repeated.skipped(), 1);
}

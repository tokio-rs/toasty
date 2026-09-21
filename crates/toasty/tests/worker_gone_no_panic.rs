//! Regression: sending an operation after the connection worker task died
//! must return a `connection_lost` error, never panic.
//!
//! Production incident (2026-09-21, rcoder test env): a PostgreSQL
//! connection blip made the driver report the connection invalid; the
//! worker closed its channel and exited (by design). A later
//! `Connection::exec_operation` on the same handle hit
//! `send(..).unwrap()` / `rx.await.unwrap()` and panicked inside a
//! storage owner that deliberately fails closed on panic — turning one
//! transient blip into a permanent "database is closing" outage. These
//! tests pin the fixed behavior: the caller observes an error it can
//! branch on (`is_connection_lost`), and the panic-catching layers stay
//! alive.
use std::{
    borrow::Cow,
    sync::Arc,
    sync::atomic::{AtomicU32, Ordering},
};

use async_trait::async_trait;
use tempfile::TempDir;
use toasty_core::{
    Result, Schema,
    driver::{Capability, ConnectContext, Connection, Driver, ExecResponse, Operation},
    schema::{
        db::{AppliedMigration, Migration},
        diff,
    },
};

#[derive(Debug, toasty::Model)]
struct User {
    #[key]
    #[auto]
    id: u64,
    name: String,
    age: i64,
}

#[derive(Debug, Default)]
struct MockState {
    exec_fail_tokens: AtomicU32,
}

#[derive(Debug)]
struct MockDriver {
    inner: toasty_driver_sqlite::Sqlite,
    state: Arc<MockState>,
    _tempdir: TempDir,
}

impl MockDriver {
    fn new() -> Self {
        let tempdir = TempDir::new().expect("create tempdir");
        let path = tempdir.path().join("worker_gone.db");
        Self {
            inner: toasty_driver_sqlite::Sqlite::open(&path),
            state: Arc::new(MockState::default()),
            _tempdir: tempdir,
        }
    }
}

#[async_trait]
impl Driver for MockDriver {
    fn url(&self) -> Cow<'_, str> {
        self.inner.url()
    }

    fn capability(&self) -> &'static Capability {
        self.inner.capability()
    }

    async fn connect(&self, cx: &ConnectContext) -> Result<Box<dyn Connection>> {
        let inner = self.inner.connect(cx).await?;
        Ok(Box::new(MockConnection {
            inner,
            state: self.state.clone(),
            valid: true,
        }))
    }

    fn generate_migration(&self, diff: &diff::Schema<'_>) -> Migration {
        self.inner.generate_migration(diff)
    }

    async fn reset_db(&self) -> Result<()> {
        self.inner.reset_db().await
    }
}

#[derive(Debug)]
struct MockConnection {
    inner: Box<dyn Connection>,
    state: Arc<MockState>,
    valid: bool,
}

#[async_trait]
impl Connection for MockConnection {
    async fn exec(&mut self, schema: &Arc<Schema>, op: Operation) -> Result<ExecResponse> {
        if self.state.exec_fail_tokens.load(Ordering::Relaxed) > 0 {
            self.state.exec_fail_tokens.fetch_sub(1, Ordering::Relaxed);
            self.valid = false;
            return Err(toasty_core::Error::connection_lost(std::io::Error::other(
                "mock fatal exec failure",
            )));
        }
        self.inner.exec(schema, op).await
    }

    fn is_valid(&self) -> bool {
        self.valid && self.inner.is_valid()
    }

    async fn ping(&mut self) -> Result<()> {
        self.inner.ping().await
    }

    async fn push_schema(&mut self, schema: &Schema) -> Result<()> {
        self.inner.push_schema(schema).await
    }

    async fn applied_migrations(&mut self) -> Result<Vec<AppliedMigration>> {
        self.inner.applied_migrations().await
    }

    async fn apply_migration(&mut self, id: u64, name: &str, migration: &Migration) -> Result<()> {
        self.inner.apply_migration(id, name, migration).await
    }
}

/// The dead-handle window: hold one dedicated `Connection`, kill its
/// worker with an injected fatal failure, then submit another operation
/// on the same handle. Before the fix this panicked with
/// `unwrap() on Err(SendError)`; it must now surface
/// `connection_lost`.
#[tokio::test]
async fn exec_after_worker_death_returns_connection_lost_not_panic() {
    let driver = MockDriver::new();
    let state = driver.state.clone();

    let db = toasty::Db::builder()
        .models(toasty::models!(User))
        .max_pool_size(1)
        .build(driver)
        .await
        .unwrap();
    db.push_schema().await.unwrap();

    // A dedicated connection keeps the same worker handle across calls.
    let mut conn = db.connection().await.unwrap();

    state.exec_fail_tokens.store(1, Ordering::Relaxed);
    let first = toasty::create!(User { name: "a", age: 1 })
        .exec(&mut conn)
        .await;
    assert!(
        first.as_ref().is_err_and(|e| e.is_connection_lost()),
        "injected fatal failure must surface as connection_lost, got {first:?}"
    );

    // The worker has exited; a further operation on this handle must be an
    // error, not a panic. Completing this test line is the regression proof.
    let second = toasty::create!(User { name: "b", age: 2 })
        .exec(&mut conn)
        .await;
    let err = second.expect_err("operation on a dead worker handle must fail");
    assert!(
        err.is_connection_lost(),
        "dead-handle submission must be connection_lost, got {err}"
    );
}

/// The pool itself stays healthy: after the dead connection is returned
/// and evicted, a fresh operation on the Db succeeds.
#[tokio::test]
async fn pool_still_serves_after_worker_death() {
    let driver = MockDriver::new();
    let state = driver.state.clone();

    let mut db = toasty::Db::builder()
        .models(toasty::models!(User))
        .max_pool_size(1)
        .build(driver)
        .await
        .unwrap();
    db.push_schema().await.unwrap();

    {
        let mut conn = db.connection().await.unwrap();
        state.exec_fail_tokens.store(1, Ordering::Relaxed);
        let _ = toasty::create!(User { name: "x", age: 9 })
            .exec(&mut conn)
            .await;
        // Second submission on the dead handle: must be an error, not panic.
        let _ = toasty::create!(User { name: "y", age: 8 })
            .exec(&mut conn)
            .await;
    }

    toasty::create!(User { name: "z", age: 7 })
        .exec(&mut db)
        .await
        .expect("pool must serve fresh connections after worker death");
}

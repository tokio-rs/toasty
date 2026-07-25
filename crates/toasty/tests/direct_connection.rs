use std::{
    borrow::Cow,
    future::Future,
    pin::pin,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
    time::Duration,
};

use async_trait::async_trait;
use tempfile::TempDir;
use toasty_core::{
    Result, Schema,
    driver::{
        Capability, ConnectContext, Connection, ConnectionStrategy, Driver, ExecResponse, Operation,
    },
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
}

#[derive(Debug, Default)]
struct State {
    connects: AtomicU32,
    execs: AtomicU32,
    active: AtomicU32,
    max_active: AtomicU32,
    connection_drops: AtomicU32,
    driver_drops: AtomicU32,
    fail_next: AtomicBool,
}

#[derive(Debug)]
struct DirectDriver {
    inner: toasty_driver_sqlite::Sqlite,
    state: Arc<State>,
    _tempdir: TempDir,
}

impl DirectDriver {
    fn new() -> Self {
        let tempdir = TempDir::new().unwrap();
        let inner = toasty_driver_sqlite::Sqlite::open(tempdir.path().join("direct.db"));
        Self {
            inner,
            state: Arc::new(State::default()),
            _tempdir: tempdir,
        }
    }

    fn state(&self) -> Arc<State> {
        self.state.clone()
    }
}

impl Drop for DirectDriver {
    fn drop(&mut self) {
        self.state.driver_drops.fetch_add(1, Ordering::Relaxed);
    }
}

#[async_trait]
impl Driver for DirectDriver {
    fn url(&self) -> Cow<'_, str> {
        Cow::Borrowed("direct:test")
    }

    fn capability(&self) -> &'static Capability {
        self.inner.capability()
    }

    fn connection_strategy(&self) -> ConnectionStrategy {
        ConnectionStrategy::Direct
    }

    async fn connect(&self, cx: &ConnectContext) -> Result<Box<dyn Connection>> {
        self.state.connects.fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(DirectConnection {
            inner: self.inner.connect(cx).await?,
            state: self.state.clone(),
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
struct DirectConnection {
    inner: Box<dyn Connection>,
    state: Arc<State>,
}

impl Drop for DirectConnection {
    fn drop(&mut self) {
        self.state.connection_drops.fetch_add(1, Ordering::Relaxed);
    }
}

#[async_trait]
impl Connection for DirectConnection {
    async fn exec(&mut self, schema: &Arc<Schema>, op: Operation) -> Result<ExecResponse> {
        if self.state.fail_next.swap(false, Ordering::Relaxed) {
            return Err(toasty_core::Error::unsupported_feature(
                "injected direct error",
            ));
        }

        self.state.execs.fetch_add(1, Ordering::Relaxed);
        let active = self.state.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.state.max_active.fetch_max(active, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(10)).await;
        let result = self.inner.exec(schema, op).await;
        self.state.active.fetch_sub(1, Ordering::SeqCst);
        result
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

async fn build_db() -> (toasty::Db, Arc<State>) {
    let driver = DirectDriver::new();
    let state = driver.state();
    let db = toasty::Db::builder()
        .models(toasty::models!(User))
        .build(driver)
        .await
        .unwrap();
    db.push_schema().await.unwrap();
    (db, state)
}

fn block_on<F: Future>(future: F) -> F::Output {
    struct ThreadWake(std::thread::Thread);

    impl Wake for ThreadWake {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }

    let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = pin!(future);

    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::park(),
        }
    }
}

#[test]
fn direct_build_does_not_require_tokio_runtime() {
    let driver = DirectDriver::new();
    let state = driver.state();
    let db = block_on(toasty::Db::builder().build(driver)).unwrap();

    assert_eq!(state.connects.load(Ordering::Relaxed), 1);
    assert!(db.pool().is_none());
}

#[tokio::test]
async fn direct_source_reuses_one_connection() {
    let (mut db, state) = build_db().await;

    toasty::sql::statement("DELETE FROM users")
        .exec(&mut db)
        .await
        .unwrap();
    toasty::sql::statement("DELETE FROM users")
        .exec(&mut db)
        .await
        .unwrap();

    assert_eq!(state.connects.load(Ordering::Relaxed), 1);
    assert_eq!(state.execs.load(Ordering::Relaxed), 2);
    assert!(db.pool().is_none());
}

#[tokio::test]
async fn cloned_handles_serialize_direct_operations() {
    let (mut db1, state) = build_db().await;
    let mut db2 = db1.clone();

    let (first, second) = tokio::join!(
        toasty::sql::statement("DELETE FROM users").exec(&mut db1),
        toasty::sql::statement("DELETE FROM users").exec(&mut db2),
    );
    first.unwrap();
    second.unwrap();

    assert_eq!(state.max_active.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn direct_errors_propagate_without_discarding_connection() {
    let (mut db, state) = build_db().await;
    state.fail_next.store(true, Ordering::Relaxed);

    let error = toasty::sql::statement("DELETE FROM users")
        .exec(&mut db)
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "unsupported feature: injected direct error"
    );

    toasty::sql::statement("DELETE FROM users")
        .exec(&mut db)
        .await
        .unwrap();
    assert_eq!(state.connects.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn final_handle_drops_direct_driver_and_connection() {
    let (db, state) = build_db().await;
    let clone = db.clone();

    drop(db);
    assert_eq!(state.connection_drops.load(Ordering::Relaxed), 0);
    assert_eq!(state.driver_drops.load(Ordering::Relaxed), 0);

    drop(clone);
    assert_eq!(state.connection_drops.load(Ordering::Relaxed), 1);
    assert_eq!(state.driver_drops.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn direct_source_rejects_pool_configuration() {
    let driver = DirectDriver::new();
    let error = toasty::Db::builder()
        .models(toasty::models!(User))
        .max_pool_size(2)
        .build(driver)
        .await
        .unwrap_err();

    assert!(error.is_invalid_driver_configuration());
    assert!(error.to_string().contains("max_pool_size"));
}

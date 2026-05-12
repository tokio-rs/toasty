//! Deterministic tests for the pool's background sweep and recycle
//! behavior.
//!
//! Each test runs on paused tokio time against a fault-injecting
//! `Driver` that wraps a file-backed SQLite. The wrapper lets the
//! test arm a queue of "next N pings fail" / "next N execs fail"
//! tokens; everything else passes through to SQLite so the engine
//! can run real schema pushes and inserts. No real-time deadlines.

use std::{
    borrow::Cow,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use tempfile::TempDir;
use toasty_core::{
    Result, Schema,
    driver::{Capability, Connection, Driver, ExecResponse, Operation},
    schema::db::{AppliedMigration, Migration, SchemaDiff},
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
    pings: AtomicU32,
    ping_fail_tokens: AtomicU32,
    exec_fail_tokens: AtomicU32,
}

#[derive(Debug)]
struct MockDriver {
    inner: toasty_driver_sqlite::Sqlite,
    state: Arc<MockState>,
    // Keeps the on-disk SQLite file alive for the lifetime of the
    // driver — the tempdir self-deletes on drop.
    _tempdir: TempDir,
}

impl MockDriver {
    fn new() -> Self {
        let tempdir = TempDir::new().expect("create tempdir");
        let path = tempdir.path().join("pool_sweep.db");
        Self {
            inner: toasty_driver_sqlite::Sqlite::open(&path),
            state: Arc::new(MockState::default()),
            _tempdir: tempdir,
        }
    }

    fn state(&self) -> Arc<MockState> {
        self.state.clone()
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

    async fn connect(&self) -> Result<Box<dyn Connection>> {
        let inner = self.inner.connect().await?;
        Ok(Box::new(MockConnection {
            inner,
            state: self.state.clone(),
            valid: true,
        }))
    }

    fn generate_migration(&self, diff: &SchemaDiff<'_>) -> Migration {
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
                "mock exec failure",
            )));
        }
        self.inner.exec(schema, op).await
    }

    fn is_valid(&self) -> bool {
        self.valid && self.inner.is_valid()
    }

    async fn ping(&mut self) -> Result<()> {
        self.state.pings.fetch_add(1, Ordering::Relaxed);
        if self.state.ping_fail_tokens.load(Ordering::Relaxed) > 0 {
            self.state.ping_fail_tokens.fetch_sub(1, Ordering::Relaxed);
            self.valid = false;
            return Err(toasty_core::Error::connection_lost(std::io::Error::other(
                "mock ping failure",
            )));
        }
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

/// Build a `Db` with the given pool configuration. Returns the `Db`
/// and a handle to the mock's fault-injection / counter state.
async fn build_db(
    max_pool_size: usize,
    health_check_interval: Option<Duration>,
) -> (toasty::Db, Arc<MockState>) {
    let driver = MockDriver::new();
    let state = driver.state();

    let db = toasty::Db::builder()
        .models(toasty::models!(User))
        .max_pool_size(max_pool_size)
        .pool_health_check_interval(health_check_interval)
        .build(driver)
        .await
        .unwrap();
    db.push_schema().await.unwrap();
    (db, state)
}

/// Passive recovery: with the sweep disabled, a user-observed
/// `connection_lost` must still drain the dead slot so the next
/// call opens a fresh connection. Exercises the `Manager::recycle`
/// path in isolation.
#[tokio::test(start_paused = true)]
async fn pool_recovers_after_connection_lost() {
    let (mut db, state) = build_db(1, None).await;

    toasty::create!(User {
        name: "alice",
        age: 30
    })
    .exec(&mut db)
    .await
    .unwrap();

    state.exec_fail_tokens.store(1, Ordering::Relaxed);

    let err = toasty::create!(User {
        name: "bob",
        age: 30
    })
    .exec(&mut db)
    .await
    .unwrap_err();
    assert!(
        err.is_connection_lost(),
        "expected connection_lost, got {err}"
    );

    toasty::create!(User {
        name: "carol",
        age: 30
    })
    .exec(&mut db)
    .await
    .unwrap();
}

/// Periodic sweep evicts a silently-broken idle connection without
/// needing a user query to trip it.
#[tokio::test(start_paused = true)]
async fn sweep_evicts_dead_idle_connection() {
    let (mut db, state) = build_db(1, Some(Duration::from_millis(50))).await;

    // Force the pool to open its one connection.
    toasty::create!(User {
        name: "alice",
        age: 30
    })
    .exec(&mut db)
    .await
    .unwrap();
    assert_eq!(db.pool().status().size, 1);

    state.ping_fail_tokens.store(1, Ordering::Relaxed);

    // Advance past the first sweep tick.
    tokio::time::sleep(Duration::from_millis(100)).await;

    assert_eq!(
        db.pool().status().size,
        0,
        "sweep did not evict the dead idle connection",
    );

    // Next user query opens a fresh connection.
    toasty::create!(User {
        name: "bob",
        age: 30
    })
    .exec(&mut db)
    .await
    .unwrap();
}

/// One user query observing `connection_lost` must trigger an eager
/// sweep that pings every remaining idle connection. Otherwise each
/// queued fault would surface as a separate user-query failure.
#[tokio::test(start_paused = true)]
async fn eager_escalation_after_observed_loss() {
    // 60-second interval so the periodic tick cannot fire during the
    // test — only the notify-driven escalation path can drain the
    // queued faults here.
    let (mut db, state) = build_db(3, Some(Duration::from_secs(60))).await;

    let c1 = db.connection().await.unwrap();
    let c2 = db.connection().await.unwrap();
    let c3 = db.connection().await.unwrap();
    drop((c1, c2, c3));
    assert_eq!(db.pool().status().size, 3);

    // One fault for the user query, two for the sweep's escalation
    // pings over the remaining idle connections.
    state.exec_fail_tokens.store(1, Ordering::Relaxed);
    state.ping_fail_tokens.store(2, Ordering::Relaxed);

    let err = toasty::create!(User {
        name: "alice",
        age: 30
    })
    .exec(&mut db)
    .await
    .unwrap_err();
    assert!(
        err.is_connection_lost(),
        "expected connection_lost, got {err}"
    );

    // Yield long enough on the virtual clock for the sweep to
    // observe the wake() and run its escalate.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Sweep must have pinged both remaining idles. With a 60s
    // periodic interval, nothing else explains this — only the
    // notify-driven escalation path can have fired.
    assert_eq!(state.pings.load(Ordering::Relaxed), 2);
    // Both queued ping faults were consumed by the escalation pass.
    assert_eq!(state.ping_fail_tokens.load(Ordering::Relaxed), 0);

    // And the system recovers: the next queries succeed against
    // fresh connections (would surface connection_lost if the bad
    // idles were still around).
    toasty::create!(User {
        name: "bob",
        age: 30
    })
    .exec(&mut db)
    .await
    .unwrap();
    toasty::create!(User {
        name: "carol",
        age: 30
    })
    .exec(&mut db)
    .await
    .unwrap();
}

/// A failing periodic ping self-wakes via `ConnectionTask::respond`,
/// which queues a sweep-notify permit before the same iteration
/// calls `escalate()`. The escalate snapshot includes that bump, so
/// on the next loop pass the queued permit must be deduped against
/// `last_serviced`. Without the dedup, every periodic-detected
/// failure produces a second escalate that re-pings every surviving
/// idle connection.
#[tokio::test(start_paused = true)]
async fn periodic_failure_does_not_redundantly_escalate() {
    let (db, state) = build_db(3, Some(Duration::from_secs(1))).await;

    let c1 = db.connection().await.unwrap();
    let c2 = db.connection().await.unwrap();
    let c3 = db.connection().await.unwrap();
    drop((c1, c2, c3));
    assert_eq!(db.pool().status().size, 3);

    state.ping_fail_tokens.store(1, Ordering::Relaxed);

    // Past the first tick (1s), well before the second (2s).
    tokio::time::sleep(Duration::from_millis(1_500)).await;

    // 1 failing periodic ping + 2 healthy escalate pings = 3. Without
    // dedup, the queued notify permit would drive a second escalate
    // over the two remaining idles, raising the count to 5.
    assert_eq!(
        state.pings.load(Ordering::Relaxed),
        3,
        "expected one escalation pass (1 fail + 2 healthy); redundant escalate would have raised this to 5",
    );
}

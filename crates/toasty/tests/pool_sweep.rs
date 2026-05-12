//! Deterministic tests for the pool's background sweep behavior.
//!
//! These run on paused tokio time against a minimal in-process mock
//! driver. Real-time sleeps and a real database are not necessary to
//! exercise the sweep state machine — the only driver entry point the
//! sweep uses is `ping`, which the mock makes instantaneous and
//! controllable.

use std::{
    borrow::Cow,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use toasty_core::{
    Result, Schema,
    driver::{Capability, Connection, Driver, ExecResponse, Operation},
    schema::db::{AppliedMigration, Migration, SchemaDiff},
};

#[derive(Debug, toasty::Model)]
struct Dummy {
    #[key]
    #[auto]
    id: u64,
}

/// Test handle exposing the bits the test needs to observe and steer
/// the mock driver: the total ping count, and a queue of "next N pings
/// should fail" tokens.
#[derive(Debug, Default)]
struct MockState {
    pings: AtomicU32,
    fail_tokens: AtomicU32,
}

#[derive(Debug)]
struct MockDriver {
    state: Arc<MockState>,
}

impl MockDriver {
    fn new() -> Self {
        Self {
            state: Arc::new(MockState::default()),
        }
    }

    fn state(&self) -> Arc<MockState> {
        self.state.clone()
    }
}

#[async_trait]
impl Driver for MockDriver {
    fn url(&self) -> Cow<'_, str> {
        Cow::Borrowed("mock://")
    }

    fn capability(&self) -> &'static Capability {
        &Capability::SQLITE
    }

    async fn connect(&self) -> Result<Box<dyn Connection>> {
        Ok(Box::new(MockConnection {
            state: self.state.clone(),
            valid: true,
        }))
    }

    fn generate_migration(&self, _: &SchemaDiff<'_>) -> Migration {
        unreachable!("mock driver does not support migrations")
    }

    async fn reset_db(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
struct MockConnection {
    state: Arc<MockState>,
    valid: bool,
}

#[async_trait]
impl Connection for MockConnection {
    async fn exec(&mut self, _: &Arc<Schema>, _: Operation) -> Result<ExecResponse> {
        unreachable!("pool-sweep tests do not issue user queries")
    }

    fn is_valid(&self) -> bool {
        self.valid
    }

    async fn ping(&mut self) -> Result<()> {
        self.state.pings.fetch_add(1, Ordering::Relaxed);
        if self.state.fail_tokens.load(Ordering::Relaxed) > 0 {
            self.state.fail_tokens.fetch_sub(1, Ordering::Relaxed);
            self.valid = false;
            return Err(toasty_core::Error::connection_lost(std::io::Error::other(
                "mock ping failure",
            )));
        }
        Ok(())
    }

    async fn push_schema(&mut self, _: &Schema) -> Result<()> {
        Ok(())
    }

    async fn applied_migrations(&mut self) -> Result<Vec<AppliedMigration>> {
        Ok(vec![])
    }

    async fn apply_migration(&mut self, _: u64, _: &str, _: &Migration) -> Result<()> {
        Ok(())
    }
}

/// A failing periodic ping self-wakes via `ConnectionTask::respond`,
/// which queues a sweep-notify permit before the same iteration calls
/// `escalate()`. The escalate snapshot includes that bump, so on the
/// next loop pass the queued permit must be deduped against
/// `last_serviced`. Without the dedup, every periodic-detected failure
/// produces a second escalate that re-pings every surviving idle
/// connection.
#[tokio::test(start_paused = true)]
async fn periodic_failure_does_not_redundantly_escalate() {
    let driver = MockDriver::new();
    let state = driver.state();

    let db = toasty::Db::builder()
        .models(toasty::models!(Dummy))
        .max_pool_size(3)
        .pool_health_check_interval(Some(Duration::from_secs(1)))
        .build(driver)
        .await
        .unwrap();

    // Hold three connections concurrently to force the pool to grow
    // to its max; drop them all so they go back to idle.
    let c1 = db.connection().await.unwrap();
    let c2 = db.connection().await.unwrap();
    let c3 = db.connection().await.unwrap();
    drop((c1, c2, c3));
    assert_eq!(db.pool().status().size, 3);

    // Arm the next ping to fail — simulates a single conn detecting
    // a dead backend. The two remaining idle pings (run as part of
    // escalate) succeed.
    state.fail_tokens.store(1, Ordering::Relaxed);

    // Advance through the first periodic tick (at t=1s) with margin
    // for the sweep to run periodic_iteration + escalate + the
    // dedup-skip pass, but well before t=2s when the next tick
    // would fire and add an unrelated healthy ping.
    tokio::time::sleep(Duration::from_millis(1_500)).await;

    // Expected: 1 failing periodic ping + 2 healthy escalate pings.
    // Without dedup the queued notify permit would drive a second
    // escalate over the two remaining idles, raising the count to 5.
    assert_eq!(
        state.pings.load(Ordering::Relaxed),
        3,
        "expected one escalation pass (1 fail + 2 healthy); redundant escalate would have raised this to 5",
    );
}

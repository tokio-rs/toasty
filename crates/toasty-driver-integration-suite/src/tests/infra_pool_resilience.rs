use std::time::Duration;

use crate::Fault;
use crate::prelude::*;

/// After a connection is lost, the next call must succeed against a
/// fresh connection — that is the issue from #678. Pinning
/// `max_pool_size = 1` makes eviction observable: the dead slot is the
/// only slot, so the post-failure success proves the pool re-opened.
#[driver_test(
    id(ID),
    requires(test_connection_pool),
    scenario(crate::scenarios::user_with_age)
)]
pub async fn pool_recovers_after_connection_lost(t: &mut Test) -> Result<()> {
    let mut db = t
        .setup_db_with(models!(User), |b| {
            b.max_pool_size(1);
            // Disable the sweep so passive detection is the only thing
            // that can recover the pool here — this test isolates the
            // Slice 1 path.
            b.pool_health_check_interval(None);
        })
        .await;

    toasty::create!(User {
        name: "alice",
        age: 30
    })
    .exec(&mut db)
    .await?;

    t.inject_fault(Fault::ConnectionLost);

    let err = toasty::create!(User {
        name: "bob",
        age: 30
    })
    .exec(&mut db)
    .await
    .unwrap_err();
    assert!(
        err.is_connection_lost(),
        "expected connection_lost, got {err}",
    );

    toasty::create!(User {
        name: "carol",
        age: 30
    })
    .exec(&mut db)
    .await?;

    let names = User::all()
        .exec(&mut db)
        .await?
        .into_iter()
        .map(|u| u.name)
        .collect::<Vec<_>>();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"alice".to_string()));
    assert!(names.contains(&"carol".to_string()));

    Ok(())
}

/// The background sweep pings idle connections and evicts the ones
/// that fail. With a single-connection pool and an injected fault, a
/// successful sweep iteration drops the pool size to zero *before*
/// any user query runs — proving the sweep, not a user query, did
/// the eviction.
#[driver_test(
    id(ID),
    requires(test_connection_pool),
    scenario(crate::scenarios::user_with_age)
)]
pub async fn sweep_evicts_dead_idle_connection(t: &mut Test) -> Result<()> {
    let mut db = t
        .setup_db_with(models!(User), |b| {
            b.max_pool_size(1);
            b.pool_health_check_interval(Some(Duration::from_millis(50)));
        })
        .await;

    // Force the pool to open its one connection.
    toasty::create!(User {
        name: "alice",
        age: 30
    })
    .exec(&mut db)
    .await?;
    assert_eq!(db.pool().status().size, 1);

    // Queue a fault so the next ping returns connection_lost. With no
    // user query running, only the sweep can consume this fault.
    t.inject_fault(Fault::ConnectionLost);

    // Poll for eviction. Sweep interval is 50ms, ping is a no-op
    // through the instrumented driver, so this should resolve fast.
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while db.pool().status().size > 0 && std::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(
        db.pool().status().size,
        0,
        "sweep did not evict the dead idle connection",
    );

    // The next user query opens a fresh connection and succeeds.
    toasty::create!(User {
        name: "bob",
        age: 30
    })
    .exec(&mut db)
    .await?;

    Ok(())
}

/// One user query observing `connection_lost` should trigger an eager
/// sweep that pings every remaining idle connection. Without
/// escalation, each queued fault would surface as a separate user-query
/// failure; with it, only the first query fails.
#[driver_test(
    id(ID),
    requires(test_connection_pool),
    scenario(crate::scenarios::user_with_age)
)]
pub async fn eager_escalation_after_observed_loss(t: &mut Test) -> Result<()> {
    let mut db = t
        .setup_db_with(models!(User), |b| {
            b.max_pool_size(3);
            // Long enough that the periodic tick cannot fire during
            // the test — only the notify-driven escalation path can
            // explain a result here.
            b.pool_health_check_interval(Some(Duration::from_secs(60)));
        })
        .await;

    // Open three connections and let them sit idle.
    let c1 = db.connection().await?;
    let c2 = db.connection().await?;
    let c3 = db.connection().await?;
    drop((c1, c2, c3));
    assert_eq!(db.pool().status().size, 3);

    // Three faults: one for the user query, two for the sweep's two
    // remaining-idle pings during escalation.
    t.inject_fault(Fault::ConnectionLost);
    t.inject_fault(Fault::ConnectionLost);
    t.inject_fault(Fault::ConnectionLost);

    // Trip the first fault. This also fires the sweep notify.
    let err = toasty::create!(User {
        name: "alice",
        age: 30
    })
    .exec(&mut db)
    .await
    .unwrap_err();
    assert!(
        err.is_connection_lost(),
        "expected connection_lost, got {err}",
    );

    // Give the runtime a chance to run the sweep's escalation pass.
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Without escalation, faults 2 and 3 would still be queued and
    // the next two user ops would each surface a separate
    // `connection_lost`. With escalation, the sweep drained them via
    // pings, so these run against fresh connections.
    toasty::create!(User {
        name: "bob",
        age: 30
    })
    .exec(&mut db)
    .await?;
    toasty::create!(User {
        name: "carol",
        age: 30
    })
    .exec(&mut db)
    .await?;

    Ok(())
}

/// A failing periodic ping calls `wake()` (via `ConnectionTask::respond`
/// flipping `is_valid` to `false`), which queues a notify permit. The
/// same iteration then calls `escalate()`, whose pre-escalate counter
/// snapshot includes that bump. On the next loop pass the queued permit
/// must be skipped — otherwise every periodic failure produces two
/// escalation passes and re-pings every surviving idle connection.
#[driver_test(
    id(ID),
    requires(test_connection_pool),
    scenario(crate::scenarios::user_with_age)
)]
pub async fn periodic_failure_does_not_redundantly_escalate(t: &mut Test) -> Result<()> {
    let db = t
        .setup_db_with(models!(User), |b| {
            b.max_pool_size(3);
            b.pool_health_check_interval(Some(Duration::from_millis(80)));
        })
        .await;

    // Open three connections and let them sit idle.
    let c1 = db.connection().await?;
    let c2 = db.connection().await?;
    let c3 = db.connection().await?;
    drop((c1, c2, c3));
    assert_eq!(db.pool().status().size, 3);

    // One fault — consumed by the first periodic ping. The two
    // remaining idle pings (run as part of the escalation pass)
    // succeed.
    t.inject_fault(Fault::ConnectionLost);

    // Wait until the one expected escalation pass has visibly happened
    // (1 failing periodic ping + 2 healthy escalate pings = 3).
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while t.log().ping_count() < 3 && std::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    // Then yield long enough for the sweep to process the queued
    // notify permit (if dedup is broken, the extra escalate lands
    // here in milliseconds) but well under one interval so a fresh
    // periodic tick cannot inflate the count.
    tokio::time::sleep(Duration::from_millis(40)).await;

    let pings = t.log().ping_count();
    assert_eq!(
        pings, 3,
        "expected one escalation pass (1 fail + 2 healthy); got {pings} pings, suggesting the notify branch ran a redundant escalate",
    );

    Ok(())
}

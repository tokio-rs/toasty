#![cfg(feature = "turso")]

use toasty_core::driver::{Driver, operation::TransactionMode};
use toasty_driver_turso::{EncryptionOpts, Turso};

struct TursoSetup;

impl TursoSetup {
    fn new() -> Self {
        TursoSetup
    }
}

#[async_trait::async_trait]
impl toasty_driver_integration_suite::Setup for TursoSetup {
    fn driver(&self) -> Box<dyn toasty_core::driver::Driver> {
        Box::new(toasty_driver_turso::Turso::in_memory())
    }

    async fn delete_table(&self, _name: &str) {
        // There is no need to delete anything since the driver operates in-memory
    }
}

// Generate all driver tests
toasty_driver_integration_suite::generate_driver_tests!(
    TursoSetup::new(),
    native_decimal: false,
    bigdecimal_implemented: false,
    decimal_arbitrary_precision: false,
    native_timestamp: false,
    native_date: false,
    native_time: false,
    native_datetime: false,
    native_array: false,
    native_ilike: false,
    vec_scalar: true,
    vec_remove: false,
    vec_pop: false,
    vec_remove_at: false,
    test_connection_pool: true,
);

/// Under `.concurrent_writes()`, two transactions racing on the same row
/// must produce a [`Error::serialization_failure`] on the losing commit so
/// callers can retry. Exercises the `BEGIN CONCURRENT` branch in
/// `Connection::exec` and the `Busy` / `BusySnapshot` / "conflict"-message
/// arms in `classify_turso_error`.
#[tokio::test]
async fn concurrent_writes_returns_serialization_failure() {
    #[derive(Debug, toasty::Model)]
    struct Counter {
        #[key]
        id: i64,
        tally: i64,
    }

    let mut db = toasty::Db::builder()
        .models(toasty::models!(Counter))
        .max_pool_size(4)
        .build(Turso::in_memory().concurrent_writes())
        .await
        .unwrap();
    db.push_schema().await.unwrap();

    toasty::create!(Counter { id: 1, tally: 0 })
        .exec(&mut db)
        .await
        .unwrap();

    let mut db_a = db.clone();
    let mut db_b = db.clone();
    let mut tx_a = db_a.transaction().await.unwrap();
    let mut tx_b = db_b.transaction().await.unwrap();

    // tx_a wins; the second update completes first.
    Counter::filter_by_id(1i64)
        .update()
        .tally(1)
        .exec(&mut tx_a)
        .await
        .unwrap();

    // tx_b must observe the conflict. Turso may surface it either at the
    // colliding `UPDATE` or at `COMMIT`; we accept both paths.
    let conflict = async {
        Counter::filter_by_id(1i64)
            .update()
            .tally(2)
            .exec(&mut tx_b)
            .await?;
        tx_b.commit().await
    }
    .await;

    tx_a.commit().await.expect("first commit must succeed");

    let err = conflict.expect_err("losing transaction must conflict");
    assert!(
        err.is_serialization_failure(),
        "expected is_serialization_failure(), got: {err}"
    );
}

/// `.mode(TransactionMode::Deferred)` under `concurrent_writes()` opts the
/// transaction *out* of `BEGIN CONCURRENT` and back into classic SQLite
/// deferred locking. The per-transaction escape hatch is the reason
/// `concurrent_writes()` is per-driver and `mode` is per-transaction.
///
/// The behavioral signature we can pin down: under classic deferred
/// locking the second transaction's update fails immediately because the
/// first writer holds the write lock — the source error message is
/// `"database is locked"` rather than the `BEGIN CONCURRENT` path's
/// `"Write-write conflict"`. Both classify as `serialization_failure`,
/// so the assertion below inspects the underlying message text to prove
/// the two paths are not the same.
#[tokio::test]
async fn deferred_mode_opts_out_of_begin_concurrent() {
    #[derive(Debug, toasty::Model)]
    struct Counter {
        #[key]
        id: i64,
        tally: i64,
    }

    let mut db = toasty::Db::builder()
        .models(toasty::models!(Counter))
        .max_pool_size(4)
        .build(Turso::in_memory().concurrent_writes())
        .await
        .unwrap();
    db.push_schema().await.unwrap();
    toasty::create!(Counter { id: 1, tally: 0 })
        .exec(&mut db)
        .await
        .unwrap();

    let mut db_a = db.clone();
    let mut db_b = db.clone();
    let mut tx_a = db_a
        .transaction_builder()
        .mode(TransactionMode::Deferred)
        .begin()
        .await
        .unwrap();
    let mut tx_b = db_b
        .transaction_builder()
        .mode(TransactionMode::Deferred)
        .begin()
        .await
        .unwrap();

    Counter::filter_by_id(1i64)
        .update()
        .tally(1)
        .exec(&mut tx_a)
        .await
        .unwrap();

    let losing = async {
        Counter::filter_by_id(1i64)
            .update()
            .tally(2)
            .exec(&mut tx_b)
            .await?;
        tx_b.commit().await
    }
    .await;

    tx_a.commit().await.expect("first commit must succeed");

    let err = losing.expect_err("losing transaction must error under deferred locking");
    let msg = err.to_string();
    assert!(
        msg.contains("locked") || msg.contains("Busy"),
        "deferred-mode conflict should reflect classic locking, got: {msg}"
    );
    assert!(
        !msg.contains("Write-write conflict"),
        "deferred-mode error should NOT be MVCC's Write-write conflict; got: {msg}"
    );
}

/// `.mode(TransactionMode::Immediate)` under `concurrent_writes()` must
/// emit `BEGIN IMMEDIATE`, which acquires the RESERVED write lock at
/// begin time. A second `BEGIN IMMEDIATE` against the same database
/// must therefore fail immediately — *not* return successfully the way
/// `BEGIN CONCURRENT` would.
#[tokio::test]
async fn immediate_mode_overrides_begin_concurrent() {
    #[derive(Debug, toasty::Model)]
    struct Counter {
        #[key]
        id: i64,
        tally: i64,
    }

    let db = toasty::Db::builder()
        .models(toasty::models!(Counter))
        .max_pool_size(4)
        .build(Turso::in_memory().concurrent_writes())
        .await
        .unwrap();
    db.push_schema().await.unwrap();

    let mut db_a = db.clone();
    let mut db_b = db.clone();
    let _tx_a = db_a
        .transaction_builder()
        .mode(TransactionMode::Immediate)
        .begin()
        .await
        .expect("first BEGIN IMMEDIATE must succeed");

    // The second BEGIN IMMEDIATE attempts to take the same RESERVED lock
    // and must fail. Under BEGIN CONCURRENT this would have succeeded
    // and any conflict would only surface later — so a successful begin
    // here would prove the override didn't take effect.
    match db_b
        .transaction_builder()
        .mode(TransactionMode::Immediate)
        .begin()
        .await
    {
        Ok(_) => panic!("second BEGIN IMMEDIATE must fail while the first holds the lock"),
        Err(err) => {
            let msg = err.to_string();
            assert!(
                msg.contains("locked") || msg.contains("Busy"),
                "expected lock-contention error, got: {msg}"
            );
        }
    }
}

/// `.mode(TransactionMode::Exclusive)` under `concurrent_writes()` must
/// emit `BEGIN EXCLUSIVE`, which takes the exclusive write lock at
/// begin time. A second `BEGIN EXCLUSIVE` against the held lock must
/// therefore fail — `BEGIN CONCURRENT` would have succeeded.
///
/// (Note: under Turso's MVCC journal, autocommit readers on a separate
/// connection still see the pre-update snapshot, so EXCLUSIVE does not
/// block readers the way classic SQLite does. The writer-side
/// exclusion is what the test pins down.)
#[tokio::test]
async fn exclusive_mode_overrides_begin_concurrent() {
    let db = toasty::Db::builder()
        .models(toasty::models!())
        .max_pool_size(4)
        .build(Turso::in_memory().concurrent_writes())
        .await
        .unwrap();
    db.push_schema().await.unwrap();

    let mut db_a = db.clone();
    let mut db_b = db.clone();
    let _tx_a = db_a
        .transaction_builder()
        .mode(TransactionMode::Exclusive)
        .begin()
        .await
        .expect("first BEGIN EXCLUSIVE must succeed");

    match db_b
        .transaction_builder()
        .mode(TransactionMode::Exclusive)
        .begin()
        .await
    {
        Ok(_) => panic!("second BEGIN EXCLUSIVE must fail while the first holds the lock"),
        Err(err) => {
            let msg = err.to_string();
            assert!(
                msg.contains("locked") || msg.contains("Busy"),
                "expected lock-contention error, got: {msg}"
            );
        }
    }
}

/// Smoke test: `Turso::file(...).experimental_encryption(opts)` must
/// wire the cipher and hexkey through to `turso::Builder` without panic.
/// Round-trip behavior is upstream Turso's responsibility — what the
/// driver guarantees is that the bundled call reaches the engine.
#[tokio::test]
async fn experimental_encryption_smoke() {
    let opts = EncryptionOpts {
        cipher: "aes256gcm".into(),
        hexkey: "0".repeat(64),
    };

    let tmp = std::env::temp_dir().join(format!(
        "toasty-turso-encryption-smoke-{}.db",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&tmp);

    let db = toasty::Db::builder()
        .models(toasty::models!())
        .build(Turso::file(&tmp).experimental_encryption(opts))
        .await
        .unwrap();
    db.push_schema().await.unwrap();

    let _ = std::fs::remove_file(&tmp);
}

/// `Turso::new` must accept both `turso::memory:` and `turso:/path/...`
/// and reject anything else. Mirrors the PostgreSQL driver's `url_encoding`
/// test in spirit: exercises the URL-parsing path that doesn't run through
/// the shared integration suite.
#[test]
fn url_scheme_parsing() {
    let mem = Turso::new("turso::memory:").expect("in-memory URL must parse");
    assert_eq!(mem.url(), "turso::memory:");

    let file = Turso::new("turso:/var/tmp/toasty.db").expect("file URL must parse");
    assert_eq!(file.url(), "turso:/var/tmp/toasty.db");

    Turso::new("sqlite::memory:").expect_err("non-turso scheme must be rejected");
    Turso::new("not a url at all").expect_err("malformed URL must be rejected");
}

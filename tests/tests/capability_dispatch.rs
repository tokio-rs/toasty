#![cfg(feature = "sqlite")]

//! Tests that the engine decides whether a plan needs a transaction from what
//! the driver says it can do, via [`Capability::interactive_transactions`].
//!
//! Each test runs SQLite behind an [`InstrumentedDriver`] that reports chosen
//! capabilities and records what the engine asks of it, so the assertions are
//! about the engine's choices rather than any backend's behaviour.

use toasty_core::driver::{Capability, Operation};
use toasty_driver_integration_suite::{InstrumentedDriver, InstrumentedHandle};

#[derive(Debug, toasty::Model)]
struct Author {
    #[key]
    #[auto]
    id: u64,

    #[index]
    name: String,

    #[has_many]
    books: toasty::Deferred<Vec<Book>>,
}

#[derive(Debug, toasty::Model)]
struct Book {
    #[key]
    #[auto]
    id: u64,

    #[index]
    title: String,

    #[index]
    author_id: u64,

    #[belongs_to(key = author_id, references = id)]
    author: toasty::Deferred<Author>,
}

/// Capabilities of a SQL backend that cannot open a transaction, so its reads
/// get no snapshot.
const WITHOUT_TRANSACTIONS: Capability = Capability {
    interactive_transactions: false,
    ..Capability::SQLITE
};

async fn setup(capability: &'static Capability) -> (toasty::Db, InstrumentedHandle) {
    let driver = InstrumentedDriver::new(Box::new(toasty_driver_sqlite::Sqlite::in_memory()))
        .with_capability(capability);
    let log = driver.handle();

    let mut builder = toasty::Db::builder();
    builder.models(toasty::models!(Author, Book));
    let db = builder.build(driver).await.unwrap();
    db.push_schema().await.unwrap();
    (db, log)
}

/// Seeds one author with two books, one statement at a time: a nested create
/// is a multi-write plan, which a driver without interactive transactions
/// rejects (see the last test).
async fn seed(db: &toasty::Db) -> u64 {
    let mut handle = db.clone();
    let author = Author::create()
        .name("Alice")
        .exec(&mut handle)
        .await
        .unwrap();
    for title in ["Alpha", "Beta"] {
        Book::create()
            .title(title)
            .author_id(author.id)
            .exec(&mut handle)
            .await
            .unwrap();
    }
    author.id
}

/// Drains the log, returning the operations the engine issued since it was
/// last cleared.
fn drain(log: &InstrumentedHandle) -> Vec<Operation> {
    let mut ops = Vec::new();
    while !log.is_empty() {
        ops.push(log.pop_op());
    }
    ops
}

fn took_transaction(ops: &[Operation]) -> bool {
    ops.iter().any(|op| matches!(op, Operation::Transaction(_)))
}

/// The default: an eager load reads under a transaction so both statements
/// see one snapshot.
#[tokio::test]
async fn read_only_plan_takes_a_transaction_by_default() {
    let (db, log) = setup(&Capability::SQLITE).await;
    let author_id = seed(&db).await;
    log.clear();

    let mut handle = db.clone();
    let author = Author::filter_by_id(author_id)
        .include(Author::fields().books())
        .get(&mut handle)
        .await
        .unwrap();
    assert_eq!(author.books.get().len(), 2);

    let ops = drain(&log);
    assert!(
        took_transaction(&ops),
        "expected a transaction, got {ops:?}"
    );
}

/// A driver that cannot offer a snapshot gets the same reads without one,
/// rather than a transaction it would have to refuse.
#[tokio::test]
async fn a_read_only_plan_skips_the_transaction_without_interactive_transactions() {
    let (db, log) = setup(&WITHOUT_TRANSACTIONS).await;
    let author_id = seed(&db).await;
    log.clear();

    let mut handle = db.clone();
    let author = Author::filter_by_id(author_id)
        .include(Author::fields().books())
        .get(&mut handle)
        .await
        .unwrap();
    assert_eq!(author.books.get().len(), 2, "the eager load still resolves");

    let ops = drain(&log);
    assert!(
        !took_transaction(&ops),
        "expected no transaction, got {ops:?}"
    );
}

/// The flag relaxes reads and nothing else. A plan that writes more than once
/// needs a transaction for atomicity, so without one the engine rejects it
/// before any statement reaches the driver, rather than committing half of
/// it — the guarantee this design is careful not to trade away.
#[tokio::test]
async fn a_writing_plan_is_rejected_without_interactive_transactions() {
    let (db, log) = setup(&WITHOUT_TRANSACTIONS).await;
    log.clear();

    let mut handle = db.clone();
    let err = Author::create()
        .name("Alice")
        .books([Book::create().title("Alpha"), Book::create().title("Beta")])
        .exec(&mut handle)
        .await
        .expect_err("a nested create writes more than once, so it must be rejected");
    assert!(err.is_unsupported_feature(), "unexpected error: {err}");

    let ops = drain(&log);
    assert!(
        ops.is_empty(),
        "no statement should reach the driver, got {ops:?}"
    );

    // Nothing was written, not even the parent row.
    assert_eq!(Author::all().count().exec(&mut handle).await.unwrap(), 0);
    assert_eq!(Book::all().count().exec(&mut handle).await.unwrap(), 0);
}

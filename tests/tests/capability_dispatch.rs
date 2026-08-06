#![cfg(feature = "sqlite")]

//! Tests that the engine dispatches a plan according to what the driver says
//! it can do, for the two capabilities a backend without transactions needs:
//! [`Capability::snapshot_reads`] and [`Capability::atomic_write_batch`].
//!
//! Each test wraps the SQLite driver in one that reports different
//! capabilities and records what it is asked to do, so the assertions are
//! about the engine's choices rather than any backend's behaviour.

use std::sync::{Arc, Mutex, OnceLock};

use async_trait::async_trait;
use toasty_core::{
    Schema,
    driver::{Capability, ConnectContext, Driver, ExecResponse, Operation},
    schema::{
        db::{AppliedMigration, Migration},
        diff,
    },
};

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

/// What the engine asked the driver to do, in order.
#[derive(Debug, PartialEq, Eq)]
enum Call {
    Transaction,
    Statement,
    /// A write set handed over together, with how many statements it held.
    Batch(usize),
}

type Log = Arc<Mutex<Vec<Call>>>;

/// Wraps the SQLite driver, reporting chosen capabilities and recording the
/// calls the engine makes.
#[derive(Debug)]
struct Probe {
    inner: toasty_driver_sqlite::Sqlite,
    capability: &'static Capability,
    log: Log,
}

impl Probe {
    /// Capabilities of a SQL backend that cannot open a transaction: reads
    /// get no snapshot, writes arrive as a set.
    fn without_transactions() -> &'static Capability {
        static CAPABILITY: OnceLock<Capability> = OnceLock::new();
        CAPABILITY.get_or_init(|| Capability {
            snapshot_reads: false,
            atomic_write_batch: true,
            ..Capability::SQLITE
        })
    }

    /// As above, but declining batches, so writes take the streamed path.
    fn without_transactions_or_batching() -> &'static Capability {
        static CAPABILITY: OnceLock<Capability> = OnceLock::new();
        CAPABILITY.get_or_init(|| Capability {
            snapshot_reads: false,
            ..Capability::SQLITE
        })
    }
}

#[async_trait]
impl Driver for Probe {
    fn url(&self) -> std::borrow::Cow<'_, str> {
        self.inner.url()
    }

    fn capability(&self) -> &'static Capability {
        self.capability
    }

    async fn connect(
        &self,
        cx: &ConnectContext,
    ) -> toasty_core::Result<Box<dyn toasty_core::Connection>> {
        Ok(Box::new(ProbeConnection {
            inner: self.inner.connect(cx).await?,
            log: self.log.clone(),
        }))
    }

    fn max_connections(&self) -> Option<usize> {
        self.inner.max_connections()
    }

    fn generate_migration(&self, schema_diff: &diff::Schema<'_>) -> Migration {
        self.inner.generate_migration(schema_diff)
    }

    async fn reset_db(&self) -> toasty_core::Result<()> {
        self.inner.reset_db().await
    }
}

#[derive(Debug)]
struct ProbeConnection {
    inner: Box<dyn toasty_core::Connection>,
    log: Log,
}

#[async_trait]
impl toasty_core::driver::Connection for ProbeConnection {
    async fn exec(
        &mut self,
        schema: &Arc<Schema>,
        op: Operation,
    ) -> toasty_core::Result<ExecResponse> {
        self.log.lock().unwrap().push(match op {
            Operation::Transaction(_) => Call::Transaction,
            _ => Call::Statement,
        });
        self.inner.exec(schema, op).await
    }

    async fn exec_batch(
        &mut self,
        schema: &Arc<Schema>,
        ops: Vec<Operation>,
    ) -> toasty_core::Result<Vec<ExecResponse>> {
        self.log.lock().unwrap().push(Call::Batch(ops.len()));

        // SQLite has real transactions; a driver reaching for this API would
        // not. Running the statements one at a time is enough to check what
        // the engine dispatched and that the results come back in order.
        let mut responses = Vec::with_capacity(ops.len());
        for op in ops {
            responses.push(self.inner.exec(schema, op).await?);
        }
        Ok(responses)
    }

    async fn push_schema(&mut self, schema: &Schema) -> toasty_core::Result<()> {
        self.inner.push_schema(schema).await
    }

    async fn applied_migrations(&mut self) -> toasty_core::Result<Vec<AppliedMigration>> {
        self.inner.applied_migrations().await
    }

    async fn apply_migration(
        &mut self,
        id: u64,
        name: &str,
        migration: &Migration,
    ) -> toasty_core::Result<()> {
        self.inner.apply_migration(id, name, migration).await
    }
}

async fn setup(capability: &'static Capability) -> (toasty::Db, Log) {
    let log: Log = Arc::new(Mutex::new(vec![]));
    let mut builder = toasty::Db::builder();
    builder.models(toasty::models!(Author, Book));
    let db = builder
        .build(Probe {
            inner: toasty_driver_sqlite::Sqlite::in_memory(),
            capability,
            log: log.clone(),
        })
        .await
        .unwrap();
    db.push_schema().await.unwrap();
    (db, log)
}

/// Clears the calls made while seeding, so an assertion sees only the
/// statement under test.
fn take(log: &Log) -> Vec<Call> {
    std::mem::take(&mut *log.lock().unwrap())
}

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

/// The default: an eager load reads under a transaction so both statements
/// see one snapshot.
#[tokio::test]
async fn read_only_plan_takes_a_transaction_by_default() {
    let (db, log) = setup(&Capability::SQLITE).await;
    let author_id = seed(&db).await;
    take(&log);

    let mut handle = db.clone();
    let author = Author::filter_by_id(author_id)
        .include(Author::fields().books())
        .get(&mut handle)
        .await
        .unwrap();
    assert_eq!(author.books.get().len(), 2);

    let calls = take(&log);
    assert!(
        calls.contains(&Call::Transaction),
        "expected a transaction, got {calls:?}"
    );
}

/// A driver that cannot offer a snapshot gets the same reads without one,
/// rather than a transaction it would have to refuse.
#[tokio::test]
async fn read_only_plan_skips_the_transaction_without_snapshot_reads() {
    let (db, log) = setup(Probe::without_transactions()).await;
    let author_id = seed(&db).await;
    take(&log);

    let mut handle = db.clone();
    let author = Author::filter_by_id(author_id)
        .include(Author::fields().books())
        .get(&mut handle)
        .await
        .unwrap();
    assert_eq!(author.books.get().len(), 2, "the eager load still resolves");

    let calls = take(&log);
    assert!(
        !calls.contains(&Call::Transaction),
        "expected no transaction, got {calls:?}"
    );
}

/// Several independent writes are handed over as one set.
#[tokio::test]
async fn writes_are_batched_when_the_driver_takes_them_together() {
    let (db, log) = setup(Probe::without_transactions()).await;
    let author_id = seed(&db).await;
    take(&log);

    let mut handle = db.clone();
    let books = toasty::create!(Book::[
        { title: "One", author_id: author_id },
        { title: "Two", author_id: author_id },
    ])
    .exec(&mut handle)
    .await
    .unwrap();

    assert_eq!(books.len(), 2);
    assert!(books.iter().all(|book| book.id > 0), "ids: {books:?}");

    let calls = take(&log);
    assert!(
        calls.contains(&Call::Batch(2)),
        "expected one batch of two, got {calls:?}"
    );
    assert!(
        !calls.contains(&Call::Transaction),
        "a batched plan opens no transaction, got {calls:?}"
    );
}

/// Without the capability the same plan streams its writes under a
/// transaction, exactly as before.
#[tokio::test]
async fn writes_stream_under_a_transaction_without_the_capability() {
    let (db, log) = setup(Probe::without_transactions_or_batching()).await;
    let author_id = seed(&db).await;
    take(&log);

    let mut handle = db.clone();
    toasty::create!(Book::[
        { title: "One", author_id: author_id },
        { title: "Two", author_id: author_id },
    ])
    .exec(&mut handle)
    .await
    .unwrap();

    let calls = take(&log);
    assert!(
        calls.contains(&Call::Transaction),
        "expected a transaction, got {calls:?}"
    );
    assert!(
        !calls.iter().any(|call| matches!(call, Call::Batch(_))),
        "expected no batch, got {calls:?}"
    );
}

/// A single write is not a set, so it needs neither a batch nor a
/// transaction.
#[tokio::test]
async fn a_lone_write_is_not_batched() {
    let (db, log) = setup(Probe::without_transactions()).await;
    let author_id = seed(&db).await;
    take(&log);

    let mut handle = db.clone();
    Book::create()
        .title("Only")
        .author_id(author_id)
        .exec(&mut handle)
        .await
        .unwrap();

    let calls = take(&log);
    assert!(
        !calls.iter().any(|call| matches!(call, Call::Batch(_))),
        "expected no batch, got {calls:?}"
    );
}

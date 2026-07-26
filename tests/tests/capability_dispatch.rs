#![cfg(feature = "sqlite")]

//! Tests that the engine decides whether a plan needs a transaction from what
//! the driver says it can do, via [`Capability::snapshot_reads`].
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
    /// Capabilities of a SQL backend that cannot open a transaction, so its
    /// reads get no snapshot.
    fn without_transactions() -> &'static Capability {
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

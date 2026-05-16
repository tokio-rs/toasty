//! Database driver interface for Toasty.
//!
//! This module defines the traits and types that database drivers must implement
//! to integrate with the Toasty query engine. The two core traits are [`Driver`]
//! (factory for connections and schema operations) and [`Connection`] (executes
//! operations against a live database session).
//!
//! The query planner inspects [`Capability`] to decide which [`Operation`]
//! variants to emit. SQL-based drivers receive [`Operation::QuerySql`] and
//! [`Operation::Insert`], while key-value drivers (e.g., DynamoDB) receive
//! [`Operation::GetByKey`], [`Operation::QueryPk`], etc. The
//! [`SchemaMutations`] sub-struct (`Capability::schema_mutations`) describes
//! what the database can do to its own schema — for example, whether
//! `ALTER COLUMN` can change a column's type — and the migration generator
//! consults it to decide between an in-place alter and a table rebuild.
//!
//! # Architecture
//!
//! ```text
//! Query Engine  ──▶  Operation  ──▶  Connection::exec()  ──▶  ExecResponse
//!                        ▲
//!                        │
//!               Driver::capability()
//! ```
//!
//! # Error classification
//!
//! The pool and the engine branch on the error variant returned from
//! [`Connection::exec`] and [`Connection::ping`]. Drivers MUST cooperate
//! with those branches:
//!
//! - A connection-level fault (closed socket, broken pipe, protocol
//!   error, end-of-stream during handshake) MUST be classified as
//!   [`crate::Error::connection_lost`]. The pool uses that signal to
//!   evict the slot and to wake the background sweep, which then pings
//!   the remaining idle connections and drops any that also fail. Any
//!   other error variant for the same condition leaks a dead connection
//!   back into the pool.
//!
//! - A retryable transaction conflict (PostgreSQL SQLSTATE `40001`,
//!   MySQL error `1213`) SHOULD be classified as
//!   [`crate::Error::serialization_failure`]. The engine does not retry
//!   automatically; the classification is propagated to user code so
//!   the caller can decide.
//!
//! - A write attempted against a read-only session (PostgreSQL
//!   `25006`, MySQL `1792`) SHOULD be classified as
//!   [`crate::Error::read_only_transaction`].
//!
//! Other backend errors are typically wrapped with
//! [`crate::Error::driver_operation_failed`].

mod capability;
pub use capability::{Capability, SchemaMutations, StorageTypes};

mod response;
pub use response::{ExecResponse, Rows};

pub mod operation;
pub use operation::{IsolationLevel, Operation};

use crate::schema::{
    Schema,
    db::{AppliedMigration, Migration, SchemaDiff},
};

use async_trait::async_trait;

use std::{borrow::Cow, fmt::Debug, sync::Arc};

/// Factory for database connections and provider of driver-level metadata.
///
/// Each database backend (SQLite, PostgreSQL, MySQL, DynamoDB) implements this
/// trait to tell Toasty what the backend supports ([`Capability`]) and to
/// create [`Connection`] instances on demand.
///
/// # Examples
///
/// ```ignore
/// use toasty_core::driver::Driver;
///
/// // Drivers are typically constructed from a connection URL:
/// let driver: Box<dyn Driver> = make_driver("sqlite::memory:").await;
/// assert!(!driver.url().is_empty());
///
/// let capability = driver.capability();
/// assert!(capability.sql);
///
/// let conn = driver.connect().await.unwrap();
/// ```
#[async_trait]
pub trait Driver: Debug + Send + Sync + 'static {
    /// Returns the URL this driver is connecting to.
    fn url(&self) -> Cow<'_, str>;

    /// Describes the driver's capability, which informs the query planner.
    fn capability(&self) -> &'static Capability;

    /// Creates a new connection to the database.
    ///
    /// This method is called by the [`Pool`] whenever a [`Connection`] is requested while none is
    /// available and there is room to create a new [`Connection`].
    async fn connect(&self) -> crate::Result<Box<dyn Connection>>;

    /// Returns the maximum number of simultaneous database connections supported. For example,
    /// this is `Some(1)` for the in-memory SQLite driver which cannot be pooled.
    fn max_connections(&self) -> Option<usize> {
        None
    }

    /// Generates a migration from a [`SchemaDiff`].
    fn generate_migration(&self, schema_diff: &SchemaDiff<'_>) -> Migration;

    /// Drops the entire database and recreates an empty one without applying migrations.
    ///
    /// Used primarily in tests to start with a clean slate.
    async fn reset_db(&self) -> crate::Result<()>;
}

/// A live database session that can execute [`Operation`]s.
///
/// Connections are obtained from [`Driver::connect`] and are managed by the
/// connection pool. All query execution flows through [`Connection::exec`],
/// which accepts an [`Operation`] and returns an [`ExecResponse`].
///
/// # Examples
///
/// ```ignore
/// use toasty_core::driver::{Connection, Operation, ExecResponse};
/// use toasty_core::driver::operation::Transaction;
///
/// // Execute a transaction start operation on a connection:
/// let response = conn.exec(&schema, Transaction::start().into()).await?;
/// ```
#[async_trait]
pub trait Connection: Debug + Send + 'static {
    /// Executes a database operation and returns the result.
    ///
    /// This is the single entry point for all database interactions. The
    /// query engine compiles user queries into [`Operation`] values and
    /// dispatches them here. The driver translates each operation into
    /// backend-specific calls and returns an [`ExecResponse`].
    async fn exec(&mut self, schema: &Arc<Schema>, plan: Operation) -> crate::Result<ExecResponse>;

    /// Cheap, synchronous, local check that the driver's client object
    /// still considers the connection open.
    ///
    /// Examples: a flag the driver flips when its background reader
    /// reports a socket close (the MySQL driver does this), an
    /// `is_closed()` accessor on the underlying client. Implementations
    /// must not block and must not perform I/O — the check runs on the
    /// hot path of every recycle and must complete in nanoseconds.
    /// Drivers that cannot answer cheaply leave this at the default and
    /// rely on the pool's [`ping`](Self::ping) sweep or the per-acquire
    /// pre-ping option to catch a dead connection.
    ///
    /// The pool consults `is_valid()` whenever a connection is returned
    /// to the idle set. A `false` result causes the slot to be dropped
    /// before another caller can pick it up; the pool then returns
    /// another idle connection or opens a fresh one. A connection is
    /// also re-checked immediately after every [`Connection::exec`]; if
    /// the operation flipped the flag (e.g. the driver classified the
    /// error as connection-lost and updated its state), the worker task
    /// exits and the slot is evicted.
    ///
    /// The default returns `true`. Drivers without a usable passive
    /// signal stay on this default and rely on the active path: an
    /// operation surfaces [`crate::Error::connection_lost`], the pool
    /// drops the slot, and the background sweep eagerly pings the rest
    /// of the idle pool.
    fn is_valid(&self) -> bool {
        true
    }

    /// Active liveness probe. The pool's background health-check sweep
    /// calls this on the longest-idle connection on every tick, and on
    /// every other idle connection when an escalation is triggered.
    /// When `pool_pre_ping` is enabled, the pool also calls it on every
    /// acquire.
    ///
    /// Drivers MUST classify a failure here as
    /// [`crate::Error::connection_lost`] rather than a generic operation
    /// error. The pool branches on that classification to drop the slot
    /// (vs. returning it to the idle set after a transient query
    /// error), and a user-observed `connection_lost` is what wakes the
    /// pool's sweep to eagerly check the rest of the pool. Returning
    /// any other error variant from `ping` will leak a dead connection
    /// back into rotation.
    ///
    /// Drivers SHOULD make this the cheapest round-trip the backend
    /// supports (`SELECT 1`, `COM_PING`, etc.). A ping that runs slower
    /// than the sweep's per-call timeout (5 seconds, internal) is
    /// treated as failed.
    ///
    /// The default returns `Ok(())` without doing any I/O. That is the
    /// right answer for drivers whose connection layer cannot fail in
    /// isolation (the in-process SQLite driver) or whose backend
    /// manages its own pool beneath this surface (DynamoDB, where each
    /// `exec` is an HTTP call with its own retry policy).
    async fn ping(&mut self) -> crate::Result<()> {
        Ok(())
    }

    /// Creates tables and indices defined in the schema on the database.
    /// TODO: This will probably use database introspection in the future.
    async fn push_schema(&mut self, _schema: &Schema) -> crate::Result<()>;

    /// Returns a list of currently applied database migrations.
    async fn applied_migrations(&mut self) -> crate::Result<Vec<AppliedMigration>>;

    /// Applies a single migration to the database and records it as applied.
    async fn apply_migration(
        &mut self,
        id: u64,
        name: &str,
        migration: &Migration,
    ) -> crate::Result<()>;
}

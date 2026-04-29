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
//! [`Operation::GetByKey`], [`Operation::QueryPk`], etc.
//!
//! # Architecture
//!
//! ```text
//! Query Engine  ──▶  Operation  ──▶  Connection::exec()  ──▶  ExecResponse
//!                        ▲
//!                        │
//!               Driver::capability()
//! ```

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

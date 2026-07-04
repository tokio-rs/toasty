#![warn(missing_docs)]

//! Toasty driver for [Turso](https://turso.tech/), an async-native,
//! SQLite-compatible database engine.
//!
//! Speaks the same SQL dialect as the [SQLite driver][toasty-driver-sqlite]
//! but uses the async Turso client. Supports file-backed and in-memory
//! databases, an optional concurrent-writes mode that uses Turso's MVCC
//! journal so transactions don't serialize on a single writer, and per-driver
//! toggles for Turso's experimental features (`experimental_encryption`,
//! `experimental_attach`, etc.) that mirror [`turso::Builder`].
//!
//! [toasty-driver-sqlite]: https://docs.rs/toasty-driver-sqlite
//!
//! # Examples
//!
//! ```
//! use toasty_driver_turso::Turso;
//!
//! // In-memory database
//! let driver = Turso::in_memory();
//!
//! // File-backed database
//! let driver = Turso::file("path/to/db");
//!
//! // Allow transactions to run concurrently instead of serializing writers
//! let driver = Turso::file("path/to/db").concurrent_writes();
//! ```

mod error;
mod value;

use error::classify_turso_error;

/// Encryption configuration for Turso. Re-exported from the upstream
/// `turso` crate so callers don't need a direct dependency on it.
pub use turso::EncryptionOpts;

/// Turso's Sync featueres.
#[cfg(feature = "sync")]
pub mod sync;

use async_trait::async_trait;
use std::{
    borrow::Cow,
    fmt,
    path::{Path, PathBuf},
    sync::Arc,
};
use toasty_core::{
    Result, Schema,
    driver::{
        Capability, Driver, ExecResponse,
        operation::{IsolationLevel, Operation, RawSqlRet, Transaction, TypedValue},
    },
    schema::{
        db::{self, Migration, Table},
        diff,
    },
    stmt,
};
use toasty_sql::{self as sql};
use tokio::sync::Mutex;
use turso::{Builder, Connection as TursoConn, Database, Statement, Value as TursoValue};
use url::Url;

enum SqlReturn {
    Count,
    Infer,
    Types(Vec<stmt::Type>),
}

const CREATE_MIGRATIONS_TABLE: &str = "\
CREATE TABLE IF NOT EXISTS __toasty_migrations (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at TEXT NOT NULL
            )";

fn create_table_stmts(schema: &db::Schema, table: &Table) -> Vec<String> {
    let serializer = sql::Serializer::sqlite(schema);

    let mut stmts =
        vec![serializer.serialize(&sql::Statement::create_table(table, &Capability::SQLITE))];

    for index in &table.indices {
        if index.primary_key {
            continue;
        }
        stmts.push(serializer.serialize(&sql::Statement::create_index(index)));
    }

    stmts
}

#[derive(Debug, Clone)]
enum TursoPath {
    File(PathBuf),
    InMemory,
}

/// Opt-in flags for Turso's experimental features. Each field mirrors a
/// `turso::Builder::experimental_*` method and is applied in
/// [`BuilderOptions::apply`] when the driver constructs a fresh
/// [`turso::Builder`] at connection time.
#[derive(Debug, Default, Clone)]
struct BuilderOptions {
    encryption: Option<EncryptionOpts>,
    attach: bool,
    custom_types: bool,
    generated_columns: bool,
    index_method: bool,
    materialized_views: bool,
    vacuum: bool,
    multiprocess_wal: bool,
    without_rowid: bool,
}

impl BuilderOptions {
    fn apply(&self, mut b: Builder) -> Builder {
        if let Some(opts) = &self.encryption {
            // Upstream requires *both* the feature flag and the
            // key/cipher to be set; collapse them into a single call so
            // callers can't get into a half-configured state.
            b = b
                .experimental_encryption(true)
                .with_encryption(opts.clone());
        }
        if self.attach {
            b = b.experimental_attach(true);
        }
        if self.custom_types {
            b = b.experimental_custom_types(true);
        }
        if self.generated_columns {
            b = b.experimental_generated_columns(true);
        }
        if self.index_method {
            b = b.experimental_index_method(true);
        }
        if self.materialized_views {
            b = b.experimental_materialized_views(true);
        }
        if self.vacuum {
            b = b.experimental_vacuum(true);
        }
        if self.multiprocess_wal {
            b = b.experimental_multiprocess_wal(true);
        }
        if self.without_rowid {
            b = b.experimental_without_rowid(true);
        }
        b
    }
}

struct TursoBase {
    path: TursoPath,
    concurrent_writes: bool,
}

impl TursoBase {
    fn from_url(url: impl Into<String>) -> Result<Self> {
        let url_str = url.into();
        let url = Url::parse(&url_str).map_err(toasty_core::Error::driver_operation_failed)?;

        if url.scheme() != "turso" {
            return Err(toasty_core::Error::invalid_connection_url(format!(
                "connection URL does not have a `turso` scheme; url={url_str}"
            )));
        }

        let path = if url.path() == ":memory:" {
            TursoPath::InMemory
        } else {
            TursoPath::File(PathBuf::from(
                percent_encoding::percent_decode(url.path().as_bytes())
                    .decode_utf8_lossy()
                    .to_string()
                    .as_str(),
            ))
        };

        Ok(Self::with_path(path))
    }

    fn in_memory() -> Self {
        Self::with_path(TursoPath::InMemory)
    }

    fn file(path: impl AsRef<Path>) -> Self {
        Self::with_path(TursoPath::File(path.as_ref().to_path_buf()))
    }

    fn capability() -> &'static Capability {
        &Capability::TURSO
    }

    fn concurrent_writes(mut self) -> Self {
        self.concurrent_writes = true;
        self
    }

    fn url(&self) -> Cow<'_, str> {
        match &self.path {
            TursoPath::InMemory => Cow::Borrowed("turso::memory:"),
            TursoPath::File(path) => Cow::Owned(format!("turso:{}", path.display())),
        }
    }

    fn path_str(&self) -> &str {
        match &self.path {
            TursoPath::File(p) => p.to_str().unwrap_or(":memory:"),
            TursoPath::InMemory => ":memory:",
        }
    }

    fn with_path(path: TursoPath) -> Self {
        Self {
            path,
            concurrent_writes: false,
        }
    }

    async fn connect(&self, conn: TursoConn) -> Result<Box<dyn toasty_core::Connection>> {
        if self.concurrent_writes {
            conn.pragma_update("journal_mode", "'mvcc'")
                .await
                .map_err(classify_turso_error)?;
        }
        Ok(Box::new(Connection {
            conn,
            default_begin_sql: if self.concurrent_writes {
                "BEGIN CONCURRENT"
            } else {
                "BEGIN"
            },
        }))
    }

    fn generate_migration(schema_diff: &diff::Schema<'_>) -> Migration {
        let statements = sql::MigrationStatement::from_diff(schema_diff, &Capability::SQLITE);

        let sql_strings: Vec<String> = statements
            .iter()
            .map(|stmt| sql::Serializer::sqlite(stmt.schema()).serialize(stmt.statement()))
            .collect();

        Migration::new_sql_with_breakpoints(&sql_strings)
    }

    async fn reset_db<T>(database: &Mutex<Option<T>>, path: &TursoPath) -> Result<()> {
        // Drop the cached Database so subsequent `connect()` calls open a
        // fresh one. For in-memory this is the only way to wipe state;
        // for file-backed databases the file is also removed below.
        database.lock().await.take();

        if let TursoPath::File(path) = path
            && path.exists()
        {
            std::fs::remove_file(path).map_err(toasty_core::Error::driver_operation_failed)?;
        }

        Ok(())
    }
}

/// A Turso [`Driver`] that opens connections to a file or in-memory database.
///
/// Experimental Turso features are exposed as `experimental_*` builder
/// methods that mirror [`turso::Builder`].
///
/// # Examples
///
/// ```
/// use toasty_driver_turso::Turso;
///
/// // In-memory database
/// let driver = Turso::in_memory();
///
/// // File-backed database
/// let driver = Turso::file("path/to/db");
///
/// // With experimental features
/// use toasty_driver_turso::EncryptionOpts;
/// let driver = Turso::file("path/to/db")
///     .experimental_encryption(EncryptionOpts {
///         cipher: "aes256gcm".into(),
///         hexkey: "<64-hex-character-key>".into(),
///     })
///     .experimental_attach(true);
/// ```
pub struct Turso {
    base: TursoBase,
    options: BuilderOptions,
    /// Shared `turso::Database` reused across every `connect()` call so that
    /// all pool slots see the same underlying database. Without this, each
    /// connection to `:memory:` would open a fresh empty database; even
    /// file-backed handles open faster after the first builder run.
    /// Cleared by [`Driver::reset_db`] so the next `connect()` starts fresh.
    database: Mutex<Option<Database>>,
}

impl Turso {
    /// Create a new Turso driver from a connection URL.
    ///
    /// The URL scheme must be `turso` (e.g. `turso::memory:` or
    /// `turso:/path/to/db`).
    pub fn new(url: impl Into<String>) -> Result<Self> {
        Ok(Self {
            base: TursoBase::from_url(url)?,
            options: BuilderOptions::default(),
            database: Mutex::new(None),
        })
    }

    /// Create an in-memory Turso database.
    pub fn in_memory() -> Self {
        TursoBase::in_memory().into()
    }

    /// Open a Turso database at the specified file path.
    pub fn file(path: impl AsRef<Path>) -> Self {
        TursoBase::file(path).into()
    }

    /// Allow transactions to run concurrently instead of serializing on a
    /// single writer.
    ///
    /// When enabled, each new connection switches to Turso's MVCC journal
    /// (`PRAGMA journal_mode = 'mvcc'`) and a transaction started with
    /// [`TransactionMode::Default`](toasty_core::driver::operation::TransactionMode::Default)
    /// — i.e. an unspecified mode — issues `BEGIN CONCURRENT`. Conflicting
    /// transactions can then fail to commit and must be retried by the
    /// caller.
    ///
    /// Callers can opt out of MVCC concurrency on a per-transaction basis by
    /// requesting a different
    /// [`TransactionMode`](toasty_core::driver::operation::TransactionMode):
    /// `Deferred` falls back to plain `BEGIN`, while `Immediate` and
    /// `Exclusive` issue `BEGIN IMMEDIATE` / `BEGIN EXCLUSIVE` respectively.
    pub fn concurrent_writes(self) -> Self {
        Self {
            base: self.base.concurrent_writes(),
            ..self
        }
    }

    /// Enable Turso's experimental encryption with the given cipher and
    /// key. Bundles `turso::Builder::experimental_encryption(true)` with
    /// `turso::Builder::with_encryption(opts)` so callers cannot enable
    /// encryption without supplying a key.
    pub fn experimental_encryption(mut self, opts: EncryptionOpts) -> Self {
        self.options.encryption = Some(opts);
        self
    }

    /// Enable Turso's experimental `ATTACH DATABASE` support. Mirrors
    /// `turso::Builder::experimental_attach`.
    pub fn experimental_attach(mut self, on: bool) -> Self {
        self.options.attach = on;
        self
    }

    /// Enable Turso's experimental custom types. Mirrors
    /// `turso::Builder::experimental_custom_types`.
    pub fn experimental_custom_types(mut self, on: bool) -> Self {
        self.options.custom_types = on;
        self
    }

    /// Enable Turso's experimental generated columns. Mirrors
    /// `turso::Builder::experimental_generated_columns`.
    pub fn experimental_generated_columns(mut self, on: bool) -> Self {
        self.options.generated_columns = on;
        self
    }

    /// Enable Turso's experimental index methods. Mirrors
    /// `turso::Builder::experimental_index_method`.
    pub fn experimental_index_method(mut self, on: bool) -> Self {
        self.options.index_method = on;
        self
    }

    /// Enable Turso's experimental materialized views. Mirrors
    /// `turso::Builder::experimental_materialized_views`.
    pub fn experimental_materialized_views(mut self, on: bool) -> Self {
        self.options.materialized_views = on;
        self
    }

    /// Enable Turso's experimental `VACUUM`. Mirrors
    /// `turso::Builder::experimental_vacuum`.
    pub fn experimental_vacuum(mut self, on: bool) -> Self {
        self.options.vacuum = on;
        self
    }

    /// Enable Turso's experimental multi-process WAL. Mirrors
    /// `turso::Builder::experimental_multiprocess_wal`.
    pub fn experimental_multiprocess_wal(mut self, on: bool) -> Self {
        self.options.multiprocess_wal = on;
        self
    }

    /// Enable Turso's experimental `WITHOUT ROWID` support. Mirrors
    /// `turso::Builder::experimental_without_rowid`.
    pub fn experimental_without_rowid(mut self, on: bool) -> Self {
        self.options.without_rowid = on;
        self
    }

    /// Returns the cached `turso::Database`, opening it on first use.
    ///
    /// All connections handed out by [`Driver::connect`] go through the
    /// same `Database` so that `:memory:` is genuinely shared across pool
    /// slots (each `Builder::new_local(":memory:").build()` would otherwise
    /// produce a fresh, empty database).
    async fn database(&self) -> Result<Database> {
        let mut slot = self.database.lock().await;
        if let Some(db) = slot.as_ref() {
            return Ok(db.clone());
        }
        let builder = self.options.apply(Builder::new_local(self.base.path_str()));
        let db = builder.build().await.map_err(classify_turso_error)?;
        *slot = Some(db.clone());
        Ok(db)
    }
}

impl From<TursoBase> for Turso {
    fn from(base: TursoBase) -> Self {
        Self {
            base,
            options: BuilderOptions::default(),
            database: Mutex::new(None),
        }
    }
}

impl fmt::Debug for Turso {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Turso")
            .field("path", &self.base.path)
            .field("concurrent_writes", &self.base.concurrent_writes)
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl Driver for Turso {
    fn url(&self) -> Cow<'_, str> {
        self.base.url()
    }

    fn capability(&self) -> &'static Capability {
        TursoBase::capability()
    }

    async fn connect(&self) -> Result<Box<dyn toasty_core::Connection>> {
        let database = self.database().await?;
        let conn = database.connect().map_err(classify_turso_error)?;
        self.base.connect(conn).await
    }

    fn generate_migration(&self, schema_diff: &diff::Schema<'_>) -> Migration {
        TursoBase::generate_migration(schema_diff)
    }

    async fn reset_db(&self) -> Result<()> {
        TursoBase::reset_db(&self.database, &self.base.path).await
    }
}

/// An open connection to a Turso database.
pub struct Connection {
    conn: TursoConn,
    /// SQL to issue for [`TransactionMode::Default`]. Resolved by the
    /// driver at `connect()` time — either `"BEGIN"` for classic
    /// deferred locking, or `"BEGIN CONCURRENT"` when the driver was
    /// configured with `concurrent_writes()`. The connection no longer
    /// needs to know which mode it was opened in; it just emits the
    /// pre-baked command.
    default_begin_sql: &'static str,
}

impl fmt::Debug for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection").finish()
    }
}

impl Connection {
    async fn exec_sql(
        &mut self,
        sql_str: &str,
        typed_params: Vec<TypedValue>,
        ret: SqlReturn,
    ) -> Result<ExecResponse> {
        tracing::debug!(db.system = "turso", db.statement = %sql_str, params = typed_params.len(), "executing SQL");

        let params: Vec<TursoValue> = typed_params
            .iter()
            .map(|tv| value::to_turso(&tv.value))
            .collect();

        let mut stmt: Statement = self
            .conn
            .prepare_cached(sql_str)
            .await
            .map_err(classify_turso_error)?;

        if matches!(ret, SqlReturn::Count) {
            let count = stmt.execute(params).await.map_err(classify_turso_error)?;

            return Ok(ExecResponse::count(count as _));
        }

        let mut rows = stmt.query(params).await.map_err(classify_turso_error)?;

        let mut values = vec![];

        loop {
            match rows.next().await {
                Ok(Some(row)) => {
                    let items = match &ret {
                        SqlReturn::Count => unreachable!(),
                        SqlReturn::Infer => {
                            let mut items = vec![];
                            for index in 0..row.column_count() {
                                let turso_val =
                                    row.get_value(index).map_err(classify_turso_error)?;
                                items.push(value::from_turso_infer(turso_val));
                            }
                            items
                        }
                        SqlReturn::Types(ret_tys) => {
                            let mut items = Vec::with_capacity(ret_tys.len());
                            for (index, ret_ty) in ret_tys.iter().enumerate() {
                                let turso_val =
                                    row.get_value(index).map_err(classify_turso_error)?;
                                items.push(value::from_turso(turso_val, ret_ty));
                            }
                            items
                        }
                    };

                    values.push(stmt::ValueRecord::from_vec(items).into());
                }
                Ok(None) => break,
                Err(err) => return Err(classify_turso_error(err)),
            }
        }

        Ok(ExecResponse::value_stream(stmt::ValueStream::from_vec(
            values,
        )))
    }
}

#[async_trait]
impl toasty_core::driver::Connection for Connection {
    async fn exec(&mut self, schema: &Arc<Schema>, op: Operation) -> Result<ExecResponse> {
        tracing::trace!(driver = "turso", op = %op.name(), "driver exec");

        let (sql, typed_params, ret_tys) = match op {
            Operation::QuerySql(op) => {
                assert!(
                    op.last_insert_id_hack.is_none(),
                    "last_insert_id_hack is MySQL-specific and should not be set for Turso"
                );
                (sql::Statement::from(op.stmt), op.params, op.ret)
            }
            Operation::RawSql(op) => {
                let ret = match op.ret {
                    RawSqlRet::None => SqlReturn::Count,
                    RawSqlRet::Infer => SqlReturn::Infer,
                    RawSqlRet::Types(types) => SqlReturn::Types(types),
                };
                return self.exec_sql(&op.sql, op.params, ret).await;
            }
            Operation::Transaction(op) => {
                if let Transaction::Start { isolation, .. } = &op
                    && !matches!(isolation, Some(IsolationLevel::Serializable) | None)
                {
                    return Err(toasty_core::Error::unsupported_feature(
                        "Turso only supports Serializable isolation",
                    ));
                }
                // `default_begin_sql` is the connection's "no opinion" BEGIN
                // — `BEGIN` for classic mode, `BEGIN CONCURRENT` for MVCC —
                // and the serializer maps the other `TransactionMode`s to
                // standard SQLite SQL.
                let sql_str =
                    sql::Serializer::sqlite_with_default_begin(&schema.db, self.default_begin_sql)
                        .serialize_transaction(&op);
                self.conn
                    .execute(&sql_str, ())
                    .await
                    .map_err(classify_turso_error)?;
                return Ok(ExecResponse::count(0));
            }
            _ => todo!("op={:#?}", op),
        };

        let ret = if sql.returning_len().is_some() {
            SqlReturn::Types(ret_tys.unwrap())
        } else {
            SqlReturn::Count
        };

        let sql_str = sql::Serializer::sqlite(&schema.db).serialize(&sql);
        self.exec_sql(&sql_str, typed_params, ret).await
    }

    async fn push_schema(&mut self, schema: &Schema) -> Result<()> {
        for table in &schema.db.tables {
            tracing::debug!(table = %table.name, "creating table");
            for sql in create_table_stmts(&schema.db, table) {
                self.conn
                    .execute(&sql, ())
                    .await
                    .map_err(classify_turso_error)?;
            }
        }

        Ok(())
    }

    async fn applied_migrations(
        &mut self,
    ) -> Result<Vec<toasty_core::schema::db::AppliedMigration>> {
        self.conn
            .execute(CREATE_MIGRATIONS_TABLE, ())
            .await
            .map_err(classify_turso_error)?;

        let mut rows = self
            .conn
            .query("SELECT id FROM __toasty_migrations ORDER BY applied_at", ())
            .await
            .map_err(classify_turso_error)?;

        let mut migrations = vec![];
        loop {
            match rows.next().await {
                Ok(Some(row)) => {
                    let val = row.get_value(0).map_err(classify_turso_error)?;
                    if let TursoValue::Integer(id) = val {
                        migrations.push(toasty_core::schema::db::AppliedMigration::new(id as u64));
                    }
                }
                Ok(None) => break,
                Err(err) => return Err(classify_turso_error(err)),
            }
        }

        Ok(migrations)
    }

    async fn apply_migration(
        &mut self,
        id: u64,
        name: &str,
        migration: &toasty_core::schema::db::Migration,
    ) -> Result<()> {
        tracing::info!(id = id, name = %name, "applying migration");

        self.conn
            .execute(CREATE_MIGRATIONS_TABLE, ())
            .await
            .map_err(classify_turso_error)?;

        self.conn
            .execute("BEGIN", ())
            .await
            .map_err(classify_turso_error)?;

        for statement in migration.statements() {
            if let Err(e) = self
                .conn
                .execute(statement, ())
                .await
                .map_err(classify_turso_error)
            {
                let _ = self.conn.execute("ROLLBACK", ()).await;
                return Err(e);
            }
        }

        if let Err(e) = self
            .conn
            .execute(
                "INSERT INTO __toasty_migrations (id, name, applied_at) VALUES (?1, ?2, datetime('now'))",
                vec![
                    TursoValue::Integer(id as i64),
                    TursoValue::Text(name.to_string()),
                ],
            )
            .await
            .map_err(classify_turso_error)
        {
            let _ = self.conn.execute("ROLLBACK", ()).await;
            return Err(e);
        }

        self.conn
            .execute("COMMIT", ())
            .await
            .map_err(classify_turso_error)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Turso, TursoPath};
    use std::path::PathBuf;

    /// The file path `Turso::new` resolves out of a `turso:` URL.
    fn file_path(url: &str) -> PathBuf {
        match Turso::new(url).unwrap().base.path {
            TursoPath::File(path) => path,
            TursoPath::InMemory => panic!("expected a file-backed database for {url}"),
        }
    }

    #[test]
    fn new_decodes_percent_encoded_path() {
        // `url::Url` stores the path percent-encoded: a space becomes `%20` and
        // non-ASCII bytes become `%XX` sequences. The driver must decode it back
        // before opening the file, otherwise it opens one whose name literally
        // contains `%20`.
        assert_eq!(
            file_path("turso:/tmp/my db.sqlite"),
            PathBuf::from("/tmp/my db.sqlite")
        );
        assert_eq!(
            file_path("turso:///tmp/my%20db.sqlite"),
            PathBuf::from("/tmp/my db.sqlite")
        );
        assert_eq!(
            file_path("turso:/tmp/d%C3%A9j%C3%A0.db"),
            PathBuf::from("/tmp/déjà.db")
        );
        // Percent-decoding, not form-decoding: a literal `+` must stay a `+`.
        assert_eq!(file_path("turso:/tmp/a+b.db"), PathBuf::from("/tmp/a+b.db"));
    }

    #[test]
    fn new_memory_url_stays_in_memory() {
        assert!(matches!(
            Turso::new("turso::memory:").unwrap().base.path,
            TursoPath::InMemory
        ));
    }
}

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

#[cfg(feature = "sync")]
pub use turso::sync::{
    DatabaseSyncStats, PartialBootstrapStrategy, PartialSyncOpts, RemoteEncryptionCipher,
};

use async_trait::async_trait;
#[cfg(feature = "sync")]
use std::future::Future;
#[cfg(feature = "sync")]
use std::time::Duration;
use std::{
    borrow::Cow,
    fmt,
    path::{Path, PathBuf},
    sync::Arc,
};
use toasty_core::{
    Result, Schema,
    driver::{
        Capability, ConnectContext, Driver, ExecResponse, QueryLogConfig,
        log::QueryLog,
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
#[cfg(feature = "sync")]
use turso::sync::{AuthTokenFn, Builder, Database};
#[cfg(not(feature = "sync"))]
use turso::{Builder, Database};
use turso::{Connection as TursoConn, Statement, Value as TursoValue};
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

/// Driver builder options applied when opening a [`turso::Database`].
#[derive(Debug, Default, Clone)]
struct BuilderOptions {
    index_method: bool,

    #[cfg(not(feature = "sync"))]
    local_options: LocalBuilderOptions,

    #[cfg(feature = "sync")]
    sync_options: SyncBuilderOptions,
}

#[cfg(not(feature = "sync"))]
impl BuilderOptions {
    fn apply(&self, mut b: Builder) -> Builder {
        b = self.local_options.apply(b);
        if self.index_method {
            b = b.experimental_index_method(true);
        }
        b
    }
}

/// Opt-in flags for Turso's experimental features. Each field mirrors a
/// `turso::Builder::experimental_*` method and is applied in
/// [`LocalBuilderOptions::apply`] when the driver constructs a fresh
/// [`turso::Builder`] at connection time.
#[cfg(not(feature = "sync"))]
#[derive(Debug, Default, Clone)]
struct LocalBuilderOptions {
    encryption: Option<EncryptionOpts>,
    attach: bool,
    custom_types: bool,
    generated_columns: bool,
    materialized_views: bool,
    vacuum: bool,
    multiprocess_wal: bool,
    without_rowid: bool,
}

#[cfg(not(feature = "sync"))]
impl LocalBuilderOptions {
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

#[cfg(feature = "sync")]
/// Sync configuration for a remote Turso database. Each field mirrors a
/// `turso::sync::Builder` method and is applied in [`BuilderOptions::apply`]
/// when the driver opens a [`turso::sync::Database`].
#[derive(Clone)]
struct SyncBuilderOptions {
    remote_url: Option<String>,
    auth_token: Option<AuthTokenFn>,
    client_name: Option<String>,
    long_poll_timeout: Option<Duration>,
    /// Matches `turso::sync::Builder::new_remote`, which defaults this to
    /// `true`.
    bootstrap_if_empty: bool,
    partial_sync_config_experimental: Option<PartialSyncOpts>,
    remote_encryption: bool,
    remote_encryption_key: Option<String>,
    remote_encryption_cipher: Option<RemoteEncryptionCipher>,
}

#[cfg(feature = "sync")]
impl Default for SyncBuilderOptions {
    fn default() -> Self {
        Self {
            remote_url: None,
            auth_token: None,
            client_name: None,
            long_poll_timeout: None,
            bootstrap_if_empty: true,
            partial_sync_config_experimental: None,
            remote_encryption: false,
            remote_encryption_key: None,
            remote_encryption_cipher: None,
        }
    }
}

#[cfg(feature = "sync")]
impl fmt::Debug for SyncBuilderOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncBuilderOptions")
            .field("remote_url", &self.remote_url)
            .field(
                "auth_token",
                &self.auth_token.as_ref().map(|_| "<callback>"),
            )
            .field("client_name", &self.client_name)
            .field("long_poll_timeout", &self.long_poll_timeout)
            .field("bootstrap_if_empty", &self.bootstrap_if_empty)
            .field(
                "partial_sync_config_experimental",
                &self.partial_sync_config_experimental,
            )
            .field("remote_encryption", &self.remote_encryption)
            .field(
                "remote_encryption_key",
                &self.remote_encryption_key.as_ref().map(|_| "<redacted>"),
            )
            .field("remote_encryption_cipher", &self.remote_encryption_cipher)
            .finish()
    }
}

#[cfg(feature = "sync")]
impl BuilderOptions {
    fn apply(&self, mut b: Builder) -> Builder {
        if let Some(remote_url) = &self.sync_options.remote_url {
            b = b.with_remote_url(remote_url)
        }
        if let Some(provider) = self.sync_options.auth_token.clone() {
            b = b.with_auth_token_fn(move || provider());
        }
        if let Some(client_name) = &self.sync_options.client_name {
            b = b.with_client_name(client_name)
        }
        if let Some(timeout) = self.sync_options.long_poll_timeout {
            b = b.with_long_poll_timeout(timeout)
        }
        if let Some(opts) = &self.sync_options.partial_sync_config_experimental {
            b = b.with_partial_sync_opts_experimental(opts.clone())
        }
        if self.sync_options.remote_encryption
            && let (Some(base64_key), Some(cipher)) = (
                &self.sync_options.remote_encryption_key,
                &self.sync_options.remote_encryption_cipher,
            )
        {
            b = b.with_remote_encryption(base64_key, *cipher)
        } else if let Some(key) = &self.sync_options.remote_encryption_key {
            b = b.with_remote_encryption_key(key);
        }
        if self.index_method {
            b = b.experimental_index_method(true);
        }
        let bootstrap =
            self.sync_options.remote_url.is_some() && self.sync_options.bootstrap_if_empty;
        b = b.bootstrap_if_empty(bootstrap);
        b
    }
}

/// A Turso [`Driver`] that opens connections to a file or in-memory database.
///
/// Experimental Turso features are exposed as `experimental_*` builder
/// methods that mirror [`turso::Builder`].
///
/// # Examples
///
/// ```rust,ignore
/// use toasty_driver_turso::Turso;
///
/// // File-backed database
/// let driver = Turso::file("path/to/db");
///
/// // With experimental features
/// use toasty_driver_turso::EncryptionOpts;
///
/// let driver = Turso::file("path/to/db")
///     .experimental_encryption(EncryptionOpts {
///         cipher: "aes256gcm".into(),
///         hexkey: "<64-hex-character-key>".into(),
///     })
///     .experimental_attach(true);
///
/// // Concurrent writes
/// let driver = Turso::file("path/to/db").concurrent_writes();
///
/// // Syncing with remote server
/// let driver = Turso::file("path/to/db")
///     .with_remote_url("<remote-url>")
///     .with_auth_token("<auth-token>");
///
/// driver.push().await?;
/// ```
#[derive(Clone)]
pub struct Turso {
    path: TursoPath,
    options: BuilderOptions,
    concurrent_writes: bool,
    /// Shared `turso::Database` reused across every `connect()` call so that
    /// all pool slots see the same underlying database. Without this, each
    /// connection to `:memory:` would open a fresh empty database; even
    /// file-backed handles open faster after the first builder run.
    /// Cleared by [`Driver::reset_db`] so the next `connect()` starts fresh.
    database: Arc<Mutex<Option<Database>>>,
}

impl Turso {
    /// Create a new Turso driver from a connection URL.
    ///
    /// The URL scheme must be `turso` (e.g. `turso::memory:` or
    /// `turso:/path/to/db`).
    pub fn new(url: impl Into<String>) -> Result<Self> {
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

    /// Create an in-memory Turso database.
    pub fn in_memory() -> Self {
        Self::with_path(TursoPath::InMemory)
    }

    /// Open a Turso database at the specified file path.
    pub fn file<P: AsRef<Path>>(path: P) -> Self {
        Self::with_path(TursoPath::File(path.as_ref().to_path_buf()))
    }

    fn with_path(path: TursoPath) -> Self {
        Self {
            path,
            options: BuilderOptions::default(),
            concurrent_writes: false,
            database: Arc::new(Mutex::new(None)),
        }
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
    pub fn concurrent_writes(mut self) -> Self {
        self.concurrent_writes = true;
        self
    }

    /// Enable Turso's experimental index methods. With the `sync` feature,
    /// mirrors `turso::sync::Builder::experimental_index_method`; otherwise
    /// mirrors `turso::Builder::experimental_index_method`.
    pub fn experimental_index_method(mut self, on: bool) -> Self {
        self.options.index_method = on;
        self
    }

    /// Enable Turso's experimental encryption with the given cipher and
    /// key. Bundles `turso::Builder::experimental_encryption(true)` with
    /// `turso::Builder::with_encryption(opts)` so callers cannot enable
    /// encryption without supplying a key.
    #[cfg(not(feature = "sync"))]
    pub fn experimental_encryption(mut self, opts: EncryptionOpts) -> Self {
        self.options.local_options.encryption = Some(opts);
        self
    }

    /// Enable Turso's experimental `ATTACH DATABASE` support. Mirrors
    /// `turso::Builder::experimental_attach`.
    #[cfg(not(feature = "sync"))]
    pub fn experimental_attach(mut self, on: bool) -> Self {
        self.options.local_options.attach = on;
        self
    }

    /// Enable Turso's experimental custom types. Mirrors
    /// `turso::Builder::experimental_custom_types`.
    #[cfg(not(feature = "sync"))]
    pub fn experimental_custom_types(mut self, on: bool) -> Self {
        self.options.local_options.custom_types = on;
        self
    }

    /// Enable Turso's experimental generated columns. Mirrors
    /// `turso::Builder::experimental_generated_columns`.
    #[cfg(not(feature = "sync"))]
    pub fn experimental_generated_columns(mut self, on: bool) -> Self {
        self.options.local_options.generated_columns = on;
        self
    }

    /// Enable Turso's experimental materialized views. Mirrors
    /// `turso::Builder::experimental_materialized_views`.
    #[cfg(not(feature = "sync"))]
    pub fn experimental_materialized_views(mut self, on: bool) -> Self {
        self.options.local_options.materialized_views = on;
        self
    }

    /// Enable Turso's experimental `VACUUM`. Mirrors
    /// `turso::Builder::experimental_vacuum`.
    #[cfg(not(feature = "sync"))]
    pub fn experimental_vacuum(mut self, on: bool) -> Self {
        self.options.local_options.vacuum = on;
        self
    }

    /// Enable Turso's experimental multi-process WAL. Mirrors
    /// `turso::Builder::experimental_multiprocess_wal`.
    #[cfg(not(feature = "sync"))]
    pub fn experimental_multiprocess_wal(mut self, on: bool) -> Self {
        self.options.local_options.multiprocess_wal = on;
        self
    }

    /// Enable Turso's experimental `WITHOUT ROWID` support. Mirrors
    /// `turso::Builder::experimental_without_rowid`.
    #[cfg(not(feature = "sync"))]
    pub fn experimental_without_rowid(mut self, on: bool) -> Self {
        self.options.local_options.without_rowid = on;
        self
    }

    /// Set the remote base URL for sync HTTP requests. Mirrors
    /// `turso::sync::Builder::with_remote_url`.
    ///
    /// Accepts `https://`, `http://` and `libsql://` URLs (`libsql://` is
    /// translated to `https://`). If omitted on a file-backed database that
    /// was previously synced, Turso loads the URL from on-disk metadata.
    #[cfg(feature = "sync")]
    pub fn with_remote_url(mut self, remote_url: impl Into<String>) -> Self {
        self.options.sync_options.remote_url = Some(remote_url.into());
        self
    }

    /// Set a static authorization token for sync HTTP requests. Mirrors
    /// `turso::sync::Builder::with_auth_token`.
    ///
    /// The token is sent as a `Bearer` header (without the prefix in this
    /// argument). Overridden by [`Self::with_auth_token_fn`] if called later.
    #[cfg(feature = "sync")]
    pub fn with_auth_token(mut self, token: impl Into<String>) -> Self {
        let token = token.into();
        self.options.sync_options.auth_token = Some(Arc::new(move || {
            let token = token.clone();
            Box::pin(async move { Ok(token) })
        }));
        self
    }

    /// Set an async callback that produces an auth token on demand. Mirrors
    /// `turso::sync::Builder::with_auth_token_fn`.
    ///
    /// The callback runs before every HTTP request, so it can return a freshly
    /// rotated token (for example from a secrets manager or OAuth refresh). If
    /// the callback returns an error, the in-flight sync operation fails with
    /// that error.
    ///
    /// Overrides any previously configured static token from
    /// [`Self::with_auth_token`].
    #[cfg(feature = "sync")]
    pub fn with_auth_token_fn<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = turso::Result<String>> + Send + 'static,
    {
        self.options.sync_options.auth_token = Some(Arc::new(move || Box::pin(f())));
        self
    }

    /// Set the client name reported to the sync engine. Mirrors
    /// `turso::sync::Builder::with_client_name`.
    ///
    /// Defaults to `turso-sync-rust` when unset.
    #[cfg(feature = "sync")]
    pub fn with_client_name(mut self, name: impl Into<String>) -> Self {
        self.options.sync_options.client_name = Some(name.into());
        self
    }

    /// Set how long to wait on the remote when polling for changes. Mirrors
    /// `turso::sync::Builder::with_long_poll_timeout`.
    #[cfg(feature = "sync")]
    pub fn with_long_poll_timeout(mut self, timeout: Duration) -> Self {
        self.options.sync_options.long_poll_timeout = Some(timeout);
        self
    }

    /// Set a base64-encoded encryption key and cipher for the remote Turso
    /// Cloud database. Mirrors `turso::sync::Builder::with_remote_encryption`.
    ///
    /// The cipher determines `reserved_bytes` for page layout during bootstrap.
    #[cfg(feature = "sync")]
    pub fn with_remote_encryption(
        mut self,
        base64_key: impl Into<String>,
        cipher: RemoteEncryptionCipher,
    ) -> Self {
        self.options.sync_options.remote_encryption = true;
        self.options.sync_options.remote_encryption_key = Some(base64_key.into());
        self.options.sync_options.remote_encryption_cipher = Some(cipher);
        self
    }

    /// Set a base64-encoded encryption key for the remote Turso Cloud database.
    /// Mirrors `turso::sync::Builder::with_remote_encryption_key`.
    ///
    /// The key is sent as the `x-turso-encryption-key` header on sync HTTP
    /// requests. For deferred sync without an initial bootstrap, prefer
    /// [`Self::with_remote_encryption`] so the cipher is set for correct
    /// `reserved_bytes` calculation.
    #[cfg(feature = "sync")]
    pub fn with_remote_encryption_key(mut self, base64_key: impl Into<String>) -> Self {
        self.options.sync_options.remote_encryption_key = Some(base64_key.into());
        self
    }

    /// Enable or disable bootstrapping an empty local database from the remote.
    /// Mirrors `turso::sync::Builder::bootstrap_if_empty`.
    ///
    /// When enabled and the local database is empty, the driver downloads
    /// schema and initial data from the remote on first open. Upstream defaults
    /// to enabled; call with `false` to skip bootstrap (for example when
    /// attaching to an existing local file).
    #[cfg(feature = "sync")]
    pub fn bootstrap_if_empty(mut self, enable: bool) -> Self {
        self.options.sync_options.bootstrap_if_empty = enable;
        self
    }

    /// Set experimental partial-sync options. Mirrors
    /// `turso::sync::Builder::with_partial_sync_opts_experimental`.
    #[cfg(feature = "sync")]
    pub fn experimental_with_partial_sync_opts(mut self, opts: PartialSyncOpts) -> Self {
        self.options.sync_options.partial_sync_config_experimental = Some(opts);
        self
    }

    /// Push local changes to the remote. Mirrors
    /// [`turso::sync::Database::push`].
    ///
    /// Operates on the shared [`turso::sync::Database`] cached by this driver,
    /// so all connections in the pool see the same pending changes.
    #[cfg(feature = "sync")]
    pub async fn push(&self) -> Result<()> {
        self.database()
            .await?
            .push()
            .await
            .map_err(classify_turso_error)
    }

    /// Pull remote changes and apply them locally. Mirrors
    /// [`turso::sync::Database::pull`].
    ///
    /// Waits for remote changes, then applies them if any exist. Returns `true`
    /// when changes were applied, `false` when the remote had nothing new.
    #[cfg(feature = "sync")]
    pub async fn pull(&self) -> Result<bool> {
        self.database()
            .await?
            .pull()
            .await
            .map_err(classify_turso_error)
    }

    /// Force a WAL checkpoint on the main database. Mirrors
    /// [`turso::sync::Database::checkpoint`].
    #[cfg(feature = "sync")]
    pub async fn checkpoint(&self) -> Result<()> {
        self.database()
            .await?
            .checkpoint()
            .await
            .map_err(classify_turso_error)
    }

    /// Return sync statistics for this database. Mirrors
    /// [`turso::sync::Database::stats`].
    #[cfg(feature = "sync")]
    pub async fn stats(&self) -> Result<DatabaseSyncStats> {
        self.database()
            .await?
            .stats()
            .await
            .map_err(classify_turso_error)
    }

    fn path_str(&self) -> &str {
        match &self.path {
            TursoPath::File(p) => p.to_str().unwrap_or(":memory:"),
            TursoPath::InMemory => ":memory:",
        }
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

        #[cfg(not(feature = "sync"))]
        let builder = self.options.apply(Builder::new_local(self.path_str()));
        #[cfg(feature = "sync")]
        let builder = self.options.apply(Builder::new_remote(self.path_str()));

        let db = builder.build().await.map_err(classify_turso_error)?;
        *slot = Some(db.clone());
        Ok(db)
    }
}

impl fmt::Debug for Turso {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Turso")
            .field("path", &self.path)
            .field("concurrent_writes", &self.concurrent_writes)
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl Driver for Turso {
    fn url(&self) -> Cow<'_, str> {
        match &self.path {
            TursoPath::InMemory => Cow::Borrowed("turso::memory:"),
            TursoPath::File(path) => Cow::Owned(format!("turso:{}", path.display())),
        }
    }

    fn capability(&self) -> &'static Capability {
        &Capability::TURSO
    }

    async fn connect(&self, cx: &ConnectContext) -> Result<Box<dyn toasty_core::Connection>> {
        let db = self.database().await?;

        #[cfg(not(feature = "sync"))]
        let conn = db.connect().map_err(classify_turso_error)?;
        #[cfg(feature = "sync")]
        let conn = db.connect().await.map_err(classify_turso_error)?;

        if self.concurrent_writes {
            // `PRAGMA journal_mode = ...` returns the new mode as a row; the
            // `execute` path errors with "unexpected row during execution"
            // on any pragma that emits one. Use `pragma_update` so the row
            // is consumed.
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
            query_log: cx.query_log,
        }))
    }

    fn generate_migration(&self, schema_diff: &diff::Schema<'_>) -> Migration {
        let statements = sql::MigrationStatement::from_diff(schema_diff, &Capability::SQLITE);

        let sql_strings: Vec<String> = statements
            .iter()
            .map(|stmt| sql::Serializer::sqlite(stmt.schema()).serialize(stmt.statement()))
            .collect();

        Migration::new_sql_with_breakpoints(&sql_strings)
    }

    async fn reset_db(&self) -> Result<()> {
        // Drop the cached Database so subsequent `connect()` calls open a
        // fresh one. For in-memory this is the only way to wipe state;
        // for file-backed databases the file is also removed below.
        self.database.lock().await.take();

        if let TursoPath::File(path) = &self.path
            && path.exists()
        {
            std::fs::remove_file(path).map_err(toasty_core::Error::driver_operation_failed)?;
        }

        Ok(())
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
    query_log: QueryLogConfig,
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
        let mut log = QueryLog::sql(
            &self.query_log,
            "turso",
            sql_str,
            typed_params.iter().map(|tv| &tv.value),
        );
        let result = self
            .exec_sql_inner(sql_str, typed_params, ret, &mut log)
            .await;
        log.finish(&result);
        result
    }

    async fn exec_sql_inner(
        &mut self,
        sql_str: &str,
        typed_params: Vec<TypedValue>,
        ret: SqlReturn,
        log: &mut QueryLog<'_>,
    ) -> Result<ExecResponse> {
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

        log.rows(values.len() as u64);
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
        match Turso::new(url).unwrap().path {
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
            Turso::new("turso::memory:").unwrap().path,
            TursoPath::InMemory
        ));
    }
}

#[cfg(all(test, feature = "sync"))]
mod sync_tests {
    use super::{Turso, TursoValue};
    use reqwest::Client;
    use serde_json::{Value, json};
    use std::time::Duration;
    use tokio::time::{Instant, sleep};

    struct TursoTestServer {
        client: Client,
        db_url: String,
    }

    impl TursoTestServer {
        pub async fn new() -> Self {
            let client = Client::new();
            let db_url = std::env::var("TOASTY_TEST_TURSO_SYNC_URL")
                .unwrap_or("http://127.0.0.1:8080".to_string());

            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if client.get(&db_url).send().await.is_ok() {
                    break;
                }
                if Instant::now() >= deadline {
                    panic!("Turso sync server did not become ready within 30s; url={db_url}");
                }
                sleep(Duration::from_millis(100)).await;
            }

            Self { client, db_url }
        }

        async fn run_sql(&self, sql: &str) -> Vec<Value> {
            let resp: Value = self
                .client
                .post(format!("{}/v2/pipeline", self.db_url))
                .json(&json!({
                    "requests": [{
                        "type": "execute",
                        "stmt": { "sql": sql }
                    }]
                }))
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap()
                .json()
                .await
                .unwrap();

            let result = &resp["results"][0];
            if result["type"] != "ok" {
                panic!("pipeline failed: {resp}");
            }
            result["response"]["result"]["rows"]
                .as_array()
                .unwrap()
                .clone()
        }
    }

    #[tokio::test]
    async fn test_sync_push_and_pull() {
        let server = TursoTestServer::new().await;
        server.run_sql("DROP TABLE IF EXISTS t").await;

        let driver = Turso::in_memory().with_remote_url(&server.db_url);
        let conn = driver.database().await.unwrap().connect().await.unwrap();

        conn.execute("DROP TABLE IF EXISTS t", ()).await.unwrap();
        conn.execute("CREATE TABLE t (x TEXT)", ()).await.unwrap();
        conn.execute("INSERT INTO t VALUES ('test'), ('test-2')", ())
            .await
            .unwrap();

        driver.push().await.unwrap();

        let rows = server.run_sql("SELECT x FROM t ORDER BY x").await;
        assert_eq!(
            rows,
            vec![
                json!([{"type": "text", "value": "test"}]),
                json!([{"type": "text", "value": "test-2"}]),
            ]
        );

        server.run_sql("INSERT INTO t VALUES ('test-3')").await;

        driver.pull().await.unwrap();

        let mut local_rows = conn.query("SELECT x FROM t ORDER BY x", ()).await.unwrap();

        let mut values = vec![];
        while let Some(row) = local_rows.next().await.unwrap() {
            if let TursoValue::Text(s) = row.get_value(0).unwrap() {
                values.push(s);
            }
        }
        assert_eq!(values, vec!["test", "test-2", "test-3"]);
    }

    #[tokio::test]
    async fn test_local_db_without_remote_url() {
        let server = TursoTestServer::new().await;
        server.run_sql("DROP TABLE IF EXISTS t").await;

        let driver = Turso::in_memory();
        let _ = driver.database().await.unwrap().connect().await.unwrap();
    }
}

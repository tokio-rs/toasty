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
//! Cargo features only make capabilities available; which engine a driver
//! opens is decided at runtime by its configuration. With the `sync`
//! feature, a driver configured with [`Turso::with_remote_url`] (or any
//! other sync option, or [`Turso::with_sync`]) opens Turso's sync engine;
//! an unconfigured driver always opens the plain local engine, feature or
//! not.
//!
//! With the `serverless` feature, the driver also connects to a remote
//! Turso Cloud database over HTTP via the [`turso_serverless`] crate. A
//! connection URL with a host selects serverless mode; a URL with only a
//! path (or `:memory:`) selects the embedded engine.
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
//!
//! ```rust,ignore
//! // Turso Cloud over HTTP (requires the `serverless` feature)
//! let driver = Turso::new("turso://my-db.aws-us-east-1.turso.io")?
//!     .with_auth_token("<token>");
//! ```

mod conn;
mod error;
mod value;

use conn::AnyConn;
#[cfg(feature = "serverless")]
use error::classify_serverless_error;
use error::classify_turso_error;

/// Encryption configuration for Turso. Re-exported from the upstream
/// `turso` crate so callers don't need a direct dependency on it.
pub use turso::EncryptionOpts;

#[cfg(feature = "sync")]
pub use turso::sync::{
    DatabaseSyncStats, PartialBootstrapStrategy, PartialSyncOpts, RemoteEncryptionCipher,
};

use async_trait::async_trait;
#[cfg(any(feature = "sync", feature = "serverless"))]
use std::future::Future;
use std::time::{Duration, Instant};
use std::{
    borrow::Cow,
    fmt,
    path::{Path, PathBuf},
    sync::Arc,
};
use toasty_core::{
    Result, Schema,
    driver::{
        Capability, ConnectContext, ConnectionUrl, Driver, ExecResponse, QueryLogConfig,
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
use turso::sync::{AuthTokenFn, Builder as SyncBuilder, Database as SyncDatabase};
use turso::{Builder, Database, Value as TursoValue};

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

/// Retries an operation while the database reports a retryable
/// conflict (busy write lock, serialization failure).
///
/// A lock-taking `BEGIN` fails fast with "busy" while contended, and the
/// serverless backend has no server-side busy timeout, so the wait
/// happens here, with backoff. Retrying is safe for the two callers: a
/// busy `BEGIN` acquired nothing, and a failed transactional batch is
/// atomic — it leaves nothing applied on either transport.
async fn retry_while_busy<T, F, Fut>(mut op: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    const RETRY_FOR: Duration = Duration::from_secs(10);

    let deadline = Instant::now() + RETRY_FOR;
    let mut delay = Duration::from_millis(10);
    loop {
        match op().await {
            Err(err) if err.is_serialization_failure() && Instant::now() < deadline => {
                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(Duration::from_millis(250));
            }
            res => return res,
        }
    }
}

async fn exec_ddl(
    conn: &AnyConn,
    statements: impl IntoIterator<Item = impl AsRef<str>>,
) -> Result<()> {
    let stmts: Vec<(String, Vec<TursoValue>)> = statements
        .into_iter()
        .map(|sql| (sql.as_ref().to_string(), vec![]))
        .collect();

    retry_while_busy(|| conn.transactional_batch(&stmts)).await
}

#[derive(Debug, Clone)]
enum TursoPath {
    File(PathBuf),
    InMemory,
    /// A remote Turso Cloud database reached over HTTP ("serverless").
    /// Holds the connection URL with any `authToken` query parameter
    /// stripped.
    #[cfg(feature = "serverless")]
    Remote(String),
}

/// Driver builder options applied when opening a database.
///
/// Local and sync options coexist; which set applies is decided at
/// [`Turso::database`] time by the driver's mode, not at compile time.
#[derive(Debug, Default, Clone)]
struct BuilderOptions {
    index_method: bool,

    local_options: LocalBuilderOptions,

    #[cfg(feature = "sync")]
    sync_options: SyncBuilderOptions,

    #[cfg(feature = "serverless")]
    serverless_options: ServerlessBuilderOptions,
}

/// Options for a remote ("serverless") database, applied when the driver
/// constructs a [`turso_serverless::Builder`].
#[cfg(feature = "serverless")]
#[derive(Default, Clone)]
struct ServerlessBuilderOptions {
    auth_token: Option<String>,
    /// Overrides `auth_token` when set, mirroring the sync engine's
    /// precedence.
    auth_token_fn: Option<turso_serverless::AuthTokenFn>,
    remote_encryption_key: Option<String>,
}

#[cfg(feature = "serverless")]
impl BuilderOptions {
    fn apply_serverless(&self, mut b: turso_serverless::Builder) -> turso_serverless::Builder {
        let opts = &self.serverless_options;
        if let Some(token_fn) = &opts.auth_token_fn {
            let token_fn = token_fn.clone();
            b = b.with_auth_token_fn(move || token_fn());
        } else if let Some(token) = &opts.auth_token {
            b = b.with_auth_token(token.clone());
        }
        if let Some(key) = &opts.remote_encryption_key {
            b = b.with_remote_encryption_key(key.clone());
        }
        b
    }
}

#[cfg(feature = "serverless")]
impl fmt::Debug for ServerlessBuilderOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerlessBuilderOptions")
            .field(
                "auth_token",
                &self.auth_token.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "auth_token_fn",
                &self.auth_token_fn.as_ref().map(|_| "<callback>"),
            )
            .field(
                "remote_encryption_key",
                &self.remote_encryption_key.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

impl BuilderOptions {
    fn apply_local(&self, mut b: Builder) -> Builder {
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

impl LocalBuilderOptions {
    /// Whether any local-engine option was configured. Used to reject
    /// configurations that combine local-only options with a remote
    /// engine (sync or serverless).
    #[cfg(any(feature = "sync", feature = "serverless"))]
    fn any_set(&self) -> bool {
        self.encryption.is_some()
            || self.attach
            || self.custom_types
            || self.generated_columns
            || self.materialized_views
            || self.vacuum
            || self.multiprocess_wal
            || self.without_rowid
    }

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
    fn apply_sync(&self, mut b: SyncBuilder) -> SyncBuilder {
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
    /// Whether the driver opens the sync engine instead of the plain
    /// local one. Set by [`Turso::with_sync`] and implied by the
    /// mode-selecting sync options (`with_remote_url`, `with_client_name`,
    /// ...) — but never by credentials.
    #[cfg(feature = "sync")]
    sync_mode: bool,
    /// Shared database handle reused across every `connect()` call so that
    /// all pool slots see the same underlying database. Without this, each
    /// connection to `:memory:` would open a fresh empty database; even
    /// file-backed handles open faster after the first builder run.
    /// Cleared by [`Driver::reset_db`] so the next `connect()` starts fresh.
    database: Arc<Mutex<Option<AnyDatabase>>>,
}

/// The engine behind a [`Turso`] driver, selected at runtime by the
/// driver's configuration: the plain local engine, the sync engine that
/// replicates to a remote database (`sync` feature), or a remote Turso
/// Cloud database reached over HTTP (`serverless` feature).
#[derive(Clone)]
enum AnyDatabase {
    Local(Database),
    #[cfg(feature = "sync")]
    Sync(SyncDatabase),
    #[cfg(feature = "serverless")]
    Serverless(turso_serverless::Database),
}

impl Turso {
    /// Create a new Turso driver from a connection URL.
    ///
    /// The URL scheme must be `turso` (e.g. `turso::memory:` or
    /// `turso:/path/to/db`).
    ///
    /// With the `serverless` feature, an authority-form URL with a host
    /// selects a remote Turso Cloud database reached over HTTP:
    /// `turso://my-db.turso.io` (the form `turso db show --url` prints).
    /// The `libsql`, `https` and `http` schemes are also accepted for
    /// remote URLs, and an `authToken` query parameter is honored as if
    /// passed to [`Self::with_auth_token`] (and stripped from the URL the
    /// driver stores and reports). Scheme-relative and path forms
    /// (`turso:todos.db`, `turso:/path/to/db`) always name local files;
    /// without the feature, authority form resolves to a file path as
    /// well.
    pub fn new(url: impl Into<String>) -> Result<Self> {
        let url_str = url.into();
        let url = ConnectionUrl::parse(&url_str)?;

        #[cfg(feature = "serverless")]
        if let Some(driver) = Self::try_remote(&url) {
            return Ok(driver);
        }

        if !url.has_scheme("turso") {
            return Err(toasty_core::Error::invalid_connection_url(format!(
                "connection URL does not have a `turso` scheme; url={url_str}"
            )));
        }

        let path = url.file_path()?;
        if path == Path::new(":memory:") {
            return Ok(Self::with_path(TursoPath::InMemory));
        }

        Ok(Self::with_path(TursoPath::File(path)))
    }

    /// Builds a serverless driver when the connection URL names a remote
    /// database: a remote scheme (`libsql`, `https`, `http`), or a
    /// `turso` URL whose authority parses as a non-empty host. An
    /// authority that is not host-shaped (for example `turso://:memory:`)
    /// falls through to the file interpretation.
    #[cfg(feature = "serverless")]
    fn try_remote(url: &ConnectionUrl<'_>) -> Option<Self> {
        let is_remote_scheme =
            url.has_scheme("libsql") || url.has_scheme("https") || url.has_scheme("http");
        let has_host = url.host().ok().flatten().is_some();
        if !is_remote_scheme && !(url.has_scheme("turso") && has_host) {
            return None;
        }

        let auth_token = url
            .query_pairs()
            .find(|(key, _)| key == "authToken")
            .map(|(_, value)| value.into_owned());

        let remote = url.as_str();
        let remote = remote.split_once('?').map_or(remote, |(base, _)| base);

        let mut driver = Self::with_path(TursoPath::Remote(remote.to_string()));
        if let Some(token) = auth_token {
            driver = driver.with_auth_token(token);
        }
        Some(driver)
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
            #[cfg(feature = "sync")]
            sync_mode: false,
            database: Arc::new(Mutex::new(None)),
        }
    }

    /// Whether a remote credential (auth token or remote encryption key)
    /// is configured, under any feature.
    fn has_remote_credentials(&self) -> bool {
        #[cfg(feature = "sync")]
        if self.options.sync_options.auth_token.is_some()
            || self.options.sync_options.remote_encryption_key.is_some()
        {
            return true;
        }
        #[cfg(feature = "serverless")]
        if self.options.serverless_options.auth_token.is_some()
            || self
                .options
                .serverless_options
                .remote_encryption_key
                .is_some()
        {
            return true;
        }
        false
    }

    /// Whether this driver opens the sync engine.
    fn is_sync(&self) -> bool {
        #[cfg(feature = "sync")]
        {
            self.sync_mode
        }
        #[cfg(not(feature = "sync"))]
        {
            false
        }
    }

    /// Open the database with the sync engine even though no sync option
    /// is set. Mirrors `turso::sync::Builder::new_remote` without a
    /// remote URL: a file that was previously synced loads its remote URL
    /// from on-disk metadata.
    ///
    /// Every mode-selecting sync option ([`Self::with_remote_url`],
    /// [`Self::with_client_name`], ...) implies this; it only needs to be
    /// called explicitly for the metadata-reopen case (credentials such as
    /// [`Self::with_auth_token`] select nothing by themselves).
    #[cfg(feature = "sync")]
    pub fn with_sync(mut self) -> Self {
        self.sync_mode = true;
        self
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
    ///
    /// On a serverless (Turso Cloud) database this is a no-op: those
    /// databases are MVCC-native and transactions already run under
    /// `BEGIN CONCURRENT` by default.
    pub fn concurrent_writes(mut self) -> Self {
        self.concurrent_writes = true;
        self
    }

    /// Enable Turso's experimental index methods. Mirrors
    /// `turso::Builder::experimental_index_method` (and its
    /// `turso::sync::Builder` counterpart when the driver is configured
    /// for sync).
    pub fn experimental_index_method(mut self, on: bool) -> Self {
        self.options.index_method = on;
        self
    }

    /// Enable Turso's experimental encryption with the given cipher and
    /// key. Bundles `turso::Builder::experimental_encryption(true)` with
    /// `turso::Builder::with_encryption(opts)` so callers cannot enable
    /// encryption without supplying a key.
    pub fn experimental_encryption(mut self, opts: EncryptionOpts) -> Self {
        self.options.local_options.encryption = Some(opts);
        self
    }

    /// Enable Turso's experimental `ATTACH DATABASE` support. Mirrors
    /// `turso::Builder::experimental_attach`.
    pub fn experimental_attach(mut self, on: bool) -> Self {
        self.options.local_options.attach = on;
        self
    }

    /// Enable Turso's experimental custom types. Mirrors
    /// `turso::Builder::experimental_custom_types`.
    pub fn experimental_custom_types(mut self, on: bool) -> Self {
        self.options.local_options.custom_types = on;
        self
    }

    /// Enable Turso's experimental generated columns. Mirrors
    /// `turso::Builder::experimental_generated_columns`.
    pub fn experimental_generated_columns(mut self, on: bool) -> Self {
        self.options.local_options.generated_columns = on;
        self
    }

    /// Enable Turso's experimental materialized views. Mirrors
    /// `turso::Builder::experimental_materialized_views`.
    pub fn experimental_materialized_views(mut self, on: bool) -> Self {
        self.options.local_options.materialized_views = on;
        self
    }

    /// Enable Turso's experimental `VACUUM`. Mirrors
    /// `turso::Builder::experimental_vacuum`.
    pub fn experimental_vacuum(mut self, on: bool) -> Self {
        self.options.local_options.vacuum = on;
        self
    }

    /// Enable Turso's experimental multi-process WAL. Mirrors
    /// `turso::Builder::experimental_multiprocess_wal`.
    pub fn experimental_multiprocess_wal(mut self, on: bool) -> Self {
        self.options.local_options.multiprocess_wal = on;
        self
    }

    /// Enable Turso's experimental `WITHOUT ROWID` support. Mirrors
    /// `turso::Builder::experimental_without_rowid`.
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
        self.sync_mode = true;
        self.options.sync_options.remote_url = Some(remote_url.into());
        self
    }

    /// Set a static authorization token for remote HTTP requests — sync
    /// requests with the `sync` feature, statement execution against Turso
    /// Cloud with the `serverless` feature. Mirrors
    /// `turso::sync::Builder::with_auth_token` and
    /// `turso_serverless::Builder::with_auth_token`.
    ///
    /// The token is sent as a `Bearer` header (without the prefix in this
    /// argument). With the `sync` feature, overridden by
    /// [`Self::with_auth_token_fn`] if called later.
    ///
    /// A token is a credential, not a mode request: it does not select an
    /// engine by itself. Configuring a token on a driver that opens a
    /// plain local database is rejected when the database opens.
    #[cfg(any(feature = "sync", feature = "serverless"))]
    pub fn with_auth_token(mut self, token: impl Into<String>) -> Self {
        let token = token.into();
        #[cfg(feature = "sync")]
        {
            let token = token.clone();
            self.options.sync_options.auth_token = Some(Arc::new(move || {
                let token = token.clone();
                Box::pin(async move { Ok(token) })
            }));
        }
        #[cfg(feature = "serverless")]
        {
            self.options.serverless_options.auth_token = Some(token);
        }
        self
    }

    /// Set an async callback that produces an auth token on demand. Mirrors
    /// `turso::sync::Builder::with_auth_token_fn` and
    /// `turso_serverless::Builder::with_auth_token_fn`.
    ///
    /// The callback runs before every HTTP request, so it can return a freshly
    /// rotated token (for example from a secrets manager or OAuth refresh). If
    /// the callback returns an error, the in-flight operation fails with
    /// that error.
    ///
    /// Like [`Self::with_auth_token`], the callback is a credential, not a
    /// mode request. Overrides any previously configured static token.
    #[cfg(any(feature = "sync", feature = "serverless"))]
    pub fn with_auth_token_fn<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = turso::Result<String>> + Send + 'static,
    {
        let f = Arc::new(f);
        #[cfg(feature = "sync")]
        {
            let f = f.clone();
            self.options.sync_options.auth_token = Some(Arc::new(move || Box::pin(f())));
        }
        #[cfg(feature = "serverless")]
        {
            self.options.serverless_options.auth_token_fn = Some(Arc::new(move || {
                let f = f.clone();
                Box::pin(async move {
                    f().await
                        .map_err(|e| turso_serverless::Error::Error(e.to_string()))
                })
            }));
        }
        self
    }

    /// Set the client name reported to the sync engine. Mirrors
    /// `turso::sync::Builder::with_client_name`.
    ///
    /// Defaults to `turso-sync-rust` when unset.
    #[cfg(feature = "sync")]
    pub fn with_client_name(mut self, name: impl Into<String>) -> Self {
        self.sync_mode = true;
        self.options.sync_options.client_name = Some(name.into());
        self
    }

    /// Set how long to wait on the remote when polling for changes. Mirrors
    /// `turso::sync::Builder::with_long_poll_timeout`.
    #[cfg(feature = "sync")]
    pub fn with_long_poll_timeout(mut self, timeout: Duration) -> Self {
        self.sync_mode = true;
        self.options.sync_options.long_poll_timeout = Some(timeout);
        self
    }

    /// Set a base64-encoded encryption key and cipher for the remote Turso
    /// Cloud database. Mirrors `turso::sync::Builder::with_remote_encryption`.
    ///
    /// The cipher determines `reserved_bytes` for page layout during bootstrap.
    ///
    /// Like [`Self::with_auth_token`], the key is a credential, not a mode
    /// request.
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

    /// Set a base64-encoded encryption key for the remote Turso Cloud
    /// database. Mirrors `turso::sync::Builder::with_remote_encryption_key`
    /// and `turso_serverless::Builder::with_remote_encryption_key`.
    ///
    /// The key is sent as the `x-turso-encryption-key` header on remote
    /// HTTP requests — sync requests with the `sync` feature, statement
    /// execution with the `serverless` feature. For deferred sync without
    /// an initial bootstrap, prefer [`Self::with_remote_encryption`] so the
    /// cipher is set for correct `reserved_bytes` calculation.
    ///
    /// Like [`Self::with_auth_token`], the key is a credential, not a mode
    /// request.
    #[cfg(any(feature = "sync", feature = "serverless"))]
    pub fn with_remote_encryption_key(mut self, base64_key: impl Into<String>) -> Self {
        let base64_key = base64_key.into();
        #[cfg(feature = "sync")]
        {
            self.options.sync_options.remote_encryption_key = Some(base64_key.clone());
        }
        #[cfg(feature = "serverless")]
        {
            self.options.serverless_options.remote_encryption_key = Some(base64_key);
        }
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
        self.sync_mode = true;
        self.options.sync_options.bootstrap_if_empty = enable;
        self
    }

    /// Set experimental partial-sync options. Mirrors
    /// `turso::sync::Builder::with_partial_sync_opts_experimental`.
    #[cfg(feature = "sync")]
    pub fn experimental_with_partial_sync_opts(mut self, opts: PartialSyncOpts) -> Self {
        self.sync_mode = true;
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
        self.sync_database()
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
        self.sync_database()
            .await?
            .pull()
            .await
            .map_err(classify_turso_error)
    }

    /// Force a WAL checkpoint on the main database. Mirrors
    /// [`turso::sync::Database::checkpoint`].
    #[cfg(feature = "sync")]
    pub async fn checkpoint(&self) -> Result<()> {
        self.sync_database()
            .await?
            .checkpoint()
            .await
            .map_err(classify_turso_error)
    }

    /// Return sync statistics for this database. Mirrors
    /// [`turso::sync::Database::stats`].
    #[cfg(feature = "sync")]
    pub async fn stats(&self) -> Result<DatabaseSyncStats> {
        self.sync_database()
            .await?
            .stats()
            .await
            .map_err(classify_turso_error)
    }

    fn path_str(&self) -> &str {
        match &self.path {
            TursoPath::File(p) => p.to_str().unwrap_or(":memory:"),
            TursoPath::InMemory => ":memory:",
            #[cfg(feature = "serverless")]
            TursoPath::Remote(url) => url,
        }
    }

    /// Returns the cached database handle, opening it on first use.
    ///
    /// All connections handed out by [`Driver::connect`] go through the
    /// same `Database` so that `:memory:` is genuinely shared across pool
    /// slots (each `Builder::new_local(":memory:").build()` would otherwise
    /// produce a fresh, empty database).
    async fn database(&self) -> Result<AnyDatabase> {
        let mut slot = self.database.lock().await;
        if let Some(db) = slot.as_ref() {
            return Ok(db.clone());
        }

        let db = match &self.path {
            #[cfg(feature = "serverless")]
            TursoPath::Remote(url) => {
                // Sync replicates a local file against a remote; a
                // serverless URL leaves no local file to replicate. The
                // combination is contradictory, not something to resolve
                // by precedence.
                if self.is_sync() {
                    return Err(toasty_core::Error::unsupported_feature(
                        "the driver is configured for sync but the connection URL names \
                         a remote Turso Cloud database; sync replicates a local file — \
                         use a file path with with_remote_url() instead",
                    ));
                }
                if self.options.local_options.any_set() || self.options.index_method {
                    return Err(toasty_core::Error::unsupported_feature(
                        "local experimental options are not supported for a serverless \
                         (remote HTTP) database",
                    ));
                }
                let builder = self
                    .options
                    .apply_serverless(turso_serverless::Builder::new_remote(url.clone()));
                AnyDatabase::Serverless(builder.build().await.map_err(classify_serverless_error)?)
            }
            _ if self.is_sync() => {
                #[cfg(feature = "sync")]
                {
                    // The sync engine has no counterpart for the local
                    // experimental toggles; reject the combination instead of
                    // silently dropping options.
                    if self.options.local_options.any_set() {
                        return Err(toasty_core::Error::unsupported_feature(
                            "local experimental options are not supported when the driver \
                             is configured for sync",
                        ));
                    }
                    let builder = self
                        .options
                        .apply_sync(SyncBuilder::new_remote(self.path_str()));
                    AnyDatabase::Sync(builder.build().await.map_err(classify_turso_error)?)
                }
                #[cfg(not(feature = "sync"))]
                unreachable!("is_sync() is false without the `sync` feature")
            }
            _ => {
                // A credential (auth token, remote encryption key) is not
                // a mode request, so it never selects an engine — but a
                // credential on a plain local database is a configuration
                // error, not something to drop silently.
                if self.has_remote_credentials() {
                    return Err(toasty_core::Error::unsupported_feature(
                        "a remote credential (auth token or encryption key) is \
                         configured but the driver opens a plain local database; a \
                         credential requires a sync remote (with_remote_url() or \
                         with_sync()) or a Turso Cloud connection URL",
                    ));
                }
                let builder = self
                    .options
                    .apply_local(Builder::new_local(self.path_str()));
                AnyDatabase::Local(builder.build().await.map_err(classify_turso_error)?)
            }
        };

        *slot = Some(db.clone());
        Ok(db)
    }

    /// Returns the sync engine handle for the sync-only operations
    /// ([`Self::push`], [`Self::pull`], ...). Errors when the driver is
    /// not configured for sync — including when it points at a serverless
    /// (remote HTTP) database, which has no local replica to sync.
    #[cfg(feature = "sync")]
    async fn sync_database(&self) -> Result<SyncDatabase> {
        match self.database().await? {
            AnyDatabase::Sync(db) => Ok(db),
            AnyDatabase::Local(_) => Err(toasty_core::Error::unsupported_feature(
                "the driver is not configured for sync; call with_remote_url() or with_sync()",
            )),
            #[cfg(feature = "serverless")]
            AnyDatabase::Serverless(_) => Err(toasty_core::Error::unsupported_feature(
                "sync operations are not available for a serverless (remote HTTP) database",
            )),
        }
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
            #[cfg(feature = "serverless")]
            TursoPath::Remote(url) => Cow::Borrowed(url),
        }
    }

    fn capability(&self) -> &'static Capability {
        &Capability::TURSO
    }

    async fn connect(&self, cx: &ConnectContext) -> Result<Box<dyn toasty_core::Connection>> {
        let conn = match self.database().await? {
            AnyDatabase::Local(db) => {
                AnyConn::Embedded(db.connect().map_err(classify_turso_error)?)
            }
            #[cfg(feature = "sync")]
            AnyDatabase::Sync(db) => {
                AnyConn::Embedded(db.connect().await.map_err(classify_turso_error)?)
            }
            #[cfg(feature = "serverless")]
            AnyDatabase::Serverless(db) => {
                AnyConn::Serverless(db.connect().map_err(classify_serverless_error)?)
            }
        };

        if self.concurrent_writes && !conn.is_serverless() {
            // `PRAGMA journal_mode = ...` returns the new mode as a row; the
            // `execute` path errors with "unexpected row during execution"
            // on any pragma that emits one. Use `pragma_update` so the row
            // is consumed.
            //
            // Serverless databases skip the pragma: Turso Cloud databases
            // created with `--tursodb` are MVCC-native and the backend
            // rejects the statement ("SQL not allowed"); `BEGIN CONCURRENT`
            // alone provides the concurrent-writes semantics there.
            conn.pragma_update("journal_mode", "'mvcc'").await?;
        }

        // Serverless databases are MVCC-native (Turso Cloud, created with
        // `--tursodb`), so transactions default to `BEGIN CONCURRENT` —
        // the same semantics `concurrent_writes()` opts into for the
        // embedded engine, which makes that flag a no-op here. Callers can
        // still choose classic locking per transaction with
        // `TransactionMode::Deferred`/`Immediate`/`Exclusive`.
        let default_begin_sql = if conn.is_serverless() || self.concurrent_writes {
            "BEGIN CONCURRENT"
        } else {
            "BEGIN"
        };

        Ok(Box::new(Connection {
            conn,
            default_begin_sql,
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
        // There is no file to delete on a remote database; drop every user
        // table instead (indexes and triggers go down with their table).
        #[cfg(feature = "serverless")]
        if let TursoPath::Remote(_) = &self.path {
            let AnyDatabase::Serverless(db) = self.database().await? else {
                unreachable!("a Remote path always opens a serverless database");
            };
            let conn = AnyConn::Serverless(db.connect().map_err(classify_serverless_error)?);

            // `__turso_%` covers tursodb-internal system tables (e.g.
            // `__turso_internal_mvcc_meta`), which cannot be dropped.
            let mut rows = conn
                .query(
                    "SELECT name FROM sqlite_master \
                     WHERE type = 'table' \
                       AND name NOT LIKE 'sqlite_%' \
                       AND name NOT LIKE '\\_\\_turso\\_%' ESCAPE '\\'",
                    vec![],
                )
                .await?;

            let mut drops = vec![];
            while let Some(row) = rows.next().await? {
                if let TursoValue::Text(name) = row.get_value(0)? {
                    drops.push(format!("DROP TABLE IF EXISTS \"{name}\""));
                }
            }

            return exec_ddl(&conn, &drops).await;
        }

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
    conn: AnyConn,
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

        let mut stmt = self.conn.prepare_cached(sql_str).await?;

        if matches!(ret, SqlReturn::Count) {
            let count = stmt.execute(params).await?;

            return Ok(ExecResponse::count(count as _));
        }

        let mut rows = stmt.query(params).await?;

        let mut values = vec![];

        while let Some(row) = rows.next().await? {
            let items = match &ret {
                SqlReturn::Count => unreachable!(),
                SqlReturn::Infer => {
                    let mut items = vec![];
                    for index in 0..row.column_count() {
                        items.push(value::from_turso_infer(row.get_value(index)?));
                    }
                    items
                }
                SqlReturn::Types(ret_tys) => {
                    let mut items = Vec::with_capacity(ret_tys.len());
                    for (index, ret_ty) in ret_tys.iter().enumerate() {
                        items.push(value::from_turso(row.get_value(index)?, ret_ty));
                    }
                    items
                }
            };

            values.push(stmt::ValueRecord::from_vec(items).into());
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
            Operation::Insert(op) => (sql::Statement::from(op.stmt), op.params, op.ret),
            Operation::QuerySql(op) => (sql::Statement::from(op.stmt), op.params, op.ret),
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
                // On serverless there is no server-side busy timeout, so a
                // lock-taking BEGIN waits here instead of surfacing every
                // transient conflict. Embedded connections keep fail-fast
                // busy semantics.
                if self.conn.is_serverless()
                    && matches!(&op, Transaction::Start { .. })
                    && sql_str.starts_with("BEGIN")
                {
                    retry_while_busy(|| self.conn.execute(&sql_str, vec![])).await?;
                } else {
                    self.conn.execute(&sql_str, vec![]).await?;
                }
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
        let mut statements = vec![];
        for table in &schema.db.tables {
            tracing::debug!(table = %table.name, "creating table");
            statements.extend(create_table_stmts(&schema.db, table));
        }

        exec_ddl(&self.conn, &statements).await
    }

    async fn applied_migrations(
        &mut self,
    ) -> Result<Vec<toasty_core::schema::db::AppliedMigration>> {
        exec_ddl(&self.conn, [CREATE_MIGRATIONS_TABLE]).await?;

        let mut rows = self
            .conn
            .query(
                "SELECT id FROM __toasty_migrations ORDER BY applied_at",
                vec![],
            )
            .await?;

        let mut migrations = vec![];
        while let Some(row) = rows.next().await? {
            if let TursoValue::Integer(id) = row.get_value(0)? {
                migrations.push(toasty_core::schema::db::AppliedMigration::new(id as u64));
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

        // The whole migration — DDL plus the parameterized bookkeeping
        // INSERT — is one atomic transactional batch: a single HTTP
        // request on the serverless transport.
        let mut stmts: Vec<(String, Vec<TursoValue>)> =
            vec![(CREATE_MIGRATIONS_TABLE.to_string(), vec![])];
        for statement in migration.statements() {
            stmts.push((statement.to_string(), vec![]));
        }
        stmts.push((
            "INSERT INTO __toasty_migrations (id, name, applied_at) VALUES (?1, ?2, datetime('now'))"
                .to_string(),
            vec![
                TursoValue::Integer(id as i64),
                TursoValue::Text(name.to_string()),
            ],
        ));

        retry_while_busy(|| self.conn.transactional_batch(&stmts)).await
    }
}

/// The driver's engine is selected by configuration, not by the `sync`
/// cargo feature: unconfigured drivers stay local, and any sync option
/// (or `with_sync()`) flips to the sync engine.
#[cfg(all(test, feature = "sync"))]
mod sync_mode_tests {
    use super::Turso;

    #[test]
    fn unconfigured_driver_stays_local() {
        assert!(!Turso::in_memory().is_sync());
        assert!(!Turso::file("/tmp/db").is_sync());
        assert!(!Turso::file("/tmp/db").concurrent_writes().is_sync());
        assert!(!Turso::file("/tmp/db").experimental_attach(true).is_sync());
    }

    #[test]
    fn sync_options_imply_sync_mode() {
        assert!(Turso::file("/tmp/db").with_sync().is_sync());
        assert!(Turso::file("/tmp/db").with_remote_url("http://x").is_sync());
        assert!(Turso::file("/tmp/db").with_client_name("c").is_sync());
    }

    /// A credential (auth token, remote encryption key) is not a mode
    /// request: it selects nothing by itself, and a credential the
    /// selected engine cannot use is rejected when the database opens.
    #[tokio::test]
    async fn credentials_do_not_imply_sync_mode() {
        let driver = Turso::file("/tmp/db").with_auth_token("t");
        assert!(!driver.is_sync());
        assert!(
            driver.database().await.is_err(),
            "a token on a plain local database must be rejected"
        );

        let driver = Turso::file("/tmp/db").with_remote_encryption_key("a2V5");
        assert!(!driver.is_sync());
        assert!(
            driver.database().await.is_err(),
            "an encryption key on a plain local database must be rejected"
        );
    }

    /// Local experimental options have no sync-engine counterpart; the
    /// combination is rejected when the database is opened.
    #[tokio::test]
    async fn local_options_with_sync_mode_are_rejected() {
        let driver = Turso::in_memory().experimental_attach(true).with_sync();
        assert!(
            driver.database().await.is_err(),
            "local experimental options must be rejected in sync mode"
        );
    }
}

#[cfg(all(test, feature = "serverless"))]
mod serverless_tests {
    use super::{Turso, TursoPath};

    /// An authority-form URL with a host selects a remote Turso Cloud
    /// database — the form `turso db show --url` prints. The `libsql`
    /// scheme is accepted as an alias; host-less forms keep resolving to
    /// local files.
    #[test]
    fn new_url_with_host_is_remote() {
        let driver = Turso::new("turso://my-db.aws-us-east-1.turso.io").unwrap();
        assert!(matches!(
            &driver.path,
            TursoPath::Remote(url) if url == "turso://my-db.aws-us-east-1.turso.io"
        ));

        let driver = Turso::new("libsql://my-db.aws-us-east-1.turso.io").unwrap();
        assert!(matches!(
            &driver.path,
            TursoPath::Remote(url) if url == "libsql://my-db.aws-us-east-1.turso.io"
        ));

        // Scheme-relative and path forms always name local files.
        assert!(matches!(
            Turso::new("turso:todos.db").unwrap().path,
            TursoPath::File(path) if path == std::path::Path::new("todos.db")
        ));
        assert!(matches!(
            Turso::new("turso:///tmp/db.sqlite").unwrap().path,
            TursoPath::File(_)
        ));
        assert!(matches!(
            Turso::new("turso://:memory:").unwrap().path,
            TursoPath::InMemory
        ));
    }

    /// An `authToken` query parameter is applied as the auth token and
    /// must never leak — not through [`Driver::url`], not through `Debug`.
    #[test]
    fn new_extracts_and_strips_auth_token() {
        use toasty_core::driver::Driver;

        let driver = Turso::new("turso://my-db.turso.io?authToken=sekrit").unwrap();
        assert_eq!(driver.url(), "turso://my-db.turso.io");
        assert_eq!(
            driver.options.serverless_options.auth_token.as_deref(),
            Some("sekrit")
        );
        assert!(!format!("{driver:?}").contains("sekrit"));
    }
}

/// With both the `sync` and `serverless` features enabled, the three
/// engines coexist and the driver's configuration picks exactly one:
/// the connection URL's shape selects serverless, sync options select
/// the sync engine, and an unconfigured driver stays local.
#[cfg(all(test, feature = "sync", feature = "serverless"))]
mod combined_mode_tests {
    use super::{AnyDatabase, Turso};

    fn remote() -> Turso {
        Turso::new("turso://my-db.aws-us-east-1.turso.io").unwrap()
    }

    /// A remote (serverless) URL wins over sync mode: `with_auth_token`
    /// implies sync for file-backed databases, but against a Turso Cloud
    /// URL the token is a serverless credential, not a request to sync.
    #[tokio::test]
    async fn remote_url_with_auth_token_is_serverless() {
        let driver = remote().with_auth_token("token");
        assert!(!driver.is_sync());

        // Building a serverless handle performs no network I/O.
        assert!(matches!(
            driver.database().await.unwrap(),
            AnyDatabase::Serverless(_)
        ));
    }

    /// Sync replicates a local file; pointing a sync-configured driver at
    /// a remote (serverless) URL is contradictory and rejected, not
    /// resolved by precedence.
    #[tokio::test]
    async fn sync_options_with_remote_url_are_rejected() {
        let driver = remote().with_sync();
        assert!(
            driver.database().await.is_err(),
            "sync mode against a serverless URL must be rejected"
        );

        let driver = remote().with_remote_url("http://elsewhere");
        assert!(
            driver.database().await.is_err(),
            "with_remote_url against a serverless URL must be rejected"
        );
    }

    #[tokio::test]
    async fn unconfigured_driver_is_local() {
        assert!(matches!(
            Turso::in_memory().database().await.unwrap(),
            AnyDatabase::Local(_)
        ));
    }

    /// The rotating-token callback follows the same credential rules as
    /// the static token (see `sync_mode_tests`): here it must feed the
    /// serverless engine, not select sync.
    #[tokio::test]
    async fn auth_token_fn_is_a_shared_credential() {
        let serverless = remote().with_auth_token_fn(|| async { Ok("tok".to_string()) });
        assert!(!serverless.is_sync());
        assert!(matches!(
            serverless.database().await.unwrap(),
            AnyDatabase::Serverless(_)
        ));
    }

    /// A remote encryption key against a serverless URL is a serverless
    /// credential (sent as the `x-turso-encryption-key` header), not a
    /// sync request.
    #[tokio::test]
    async fn remote_encryption_key_passes_through_for_serverless() {
        let driver = remote().with_remote_encryption_key("a2V5");
        assert!(!driver.is_sync(), "an encryption key is not a mode request");
        assert!(
            matches!(driver.database().await.unwrap(), AnyDatabase::Serverless(_)),
            "the key must ride along to the serverless engine"
        );
    }

    /// Sync operations must be rejected for a serverless database even
    /// when the `sync` feature is compiled in.
    #[tokio::test]
    async fn sync_operations_rejected_for_serverless() {
        let driver = remote().with_auth_token("token");
        assert!(
            driver.sync_database().await.is_err(),
            "a serverless database has no local replica to sync"
        );
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
                    panic!("Turso sync server did not become ready within 5s; url={db_url}");
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
        let conn = driver
            .sync_database()
            .await
            .unwrap()
            .connect()
            .await
            .unwrap();

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

    /// With the `sync` feature enabled but no sync option configured, the
    /// driver must open the plain local engine — enabling the feature is
    /// not supposed to change behavior.
    #[tokio::test]
    async fn test_local_db_without_remote_url() {
        let driver = Turso::in_memory();
        match driver.database().await.unwrap() {
            super::AnyDatabase::Local(db) => {
                db.connect().unwrap();
            }
            _ => panic!("an unconfigured driver must open the local engine"),
        }

        assert!(
            driver.sync_database().await.is_err(),
            "sync operations must be rejected when the driver is not configured for sync"
        );
    }
}

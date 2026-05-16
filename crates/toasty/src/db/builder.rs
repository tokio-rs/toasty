use crate::{
    Db, Result,
    db::{Connect, Pool, Shared, pool::PoolConfig},
    engine::Engine,
};

use toasty_core::{
    driver::Driver,
    schema::{
        self,
        app::{self, ModelSet},
    },
};

use std::{sync::Arc, time::Duration};

/// Configures the schema and driver for a [`Db`] instance.
///
/// Provide model types with [`models`](Self::models) (using the
/// [`models!`](crate::models!) macro), optionally set a table name prefix with
/// [`table_name_prefix`](Self::table_name_prefix), then call
/// [`connect`](Self::connect) (URL-based) or [`build`](Self::build) (driver
/// instance) to open the database.
///
/// # Examples
///
/// ```
/// # tokio::runtime::Runtime::new().unwrap().block_on(async {
/// # #[derive(Debug, toasty::Model)]
/// # struct User {
/// #     #[key]
/// #     id: i64,
/// #     name: String,
/// # }
/// let driver = toasty_driver_sqlite::Sqlite::in_memory();
/// let db = toasty::Db::builder()
///     .models(toasty::models!(User))
///     .build(driver)
///     .await
///     .unwrap();
/// # });
/// ```
#[derive(Default)]
pub struct Builder {
    /// Model definitions from macro
    ///
    /// TODO: move this into `core::schema::Builder` after old schema file
    /// implementatin is removed.
    models: app::ModelSet,

    /// Schema builder
    core: schema::Builder,

    /// Connection pool configuration
    pool: PoolConfig,
}

impl Builder {
    /// Set the models whose schemas will be included when the database is
    /// built. Related and embedded models are discovered automatically through
    /// field traversal, so you only need to list your entry-point models.
    ///
    /// Use the [`models!`](crate::models!) macro to build a [`ModelSet`] from
    /// your `#[derive(Model)]` types.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// let mut builder = toasty::Db::builder();
    /// builder.models(toasty::models!(User));
    /// ```
    pub fn models(&mut self, models: ModelSet) -> &mut Self {
        self.models = models;
        self
    }

    /// Set the table name prefix for all tables
    pub fn table_name_prefix(&mut self, prefix: &str) -> &mut Self {
        self.core.table_name_prefix(prefix);
        self
    }

    /// Set the maximum number of connections the pool will maintain.
    ///
    /// Defaults to `num_cpus * 2`.
    ///
    /// Drivers may cap this below the requested value. For example, an
    /// in-memory SQLite database forces a single connection. When that
    /// happens a warning is emitted and the driver cap is used.
    pub fn max_pool_size(&mut self, max_size: usize) -> &mut Self {
        self.pool.max_size = max_size;
        self
    }

    /// Set the maximum time to wait for a free connection from the pool
    /// before returning an error. Passing `None` disables the timeout,
    /// which is the default.
    pub fn pool_wait_timeout(&mut self, timeout: Option<Duration>) -> &mut Self {
        self.pool.timeouts.wait = timeout;
        self
    }

    /// Set the maximum time allowed for establishing a new database
    /// connection. Passing `None` disables the timeout, which is the
    /// default.
    pub fn pool_create_timeout(&mut self, timeout: Option<Duration>) -> &mut Self {
        self.pool.timeouts.create = timeout;
        self
    }

    /// Configure how often the pool's background sweep pings an idle
    /// connection to detect a silently-broken backend (a database
    /// restart, a load-balancer-closed socket, a session timeout). On
    /// success the connection is returned as most-recently-used; on
    /// failure the pool eagerly pings every other idle connection and
    /// drops the ones that fail, so a single bad result drains every
    /// dead connection in one pass.
    ///
    /// The same eager sweep also fires when a user query observes
    /// [`Error::is_connection_lost`](toasty_core::Error::is_connection_lost)
    /// — so a restart usually costs at most one failed query rather
    /// than one per pooled connection.
    ///
    /// Defaults to `Some(60s)`. Pass `None` to disable the sweep and
    /// rely on passive error-driven recovery only.
    pub fn pool_health_check_interval(&mut self, interval: Option<Duration>) -> &mut Self {
        self.pool.health_check_interval = interval;
        self
    }

    /// Validate every connection with an active ping before handing it
    /// to the caller. Useful for deployments that cannot tolerate even
    /// one failed user query — a public API behind a flaky network, an
    /// idempotent worker that does not implement retry. A failing ping
    /// evicts the connection and the pool reuses another idle slot or
    /// opens a fresh one; the caller sees either a healthy connection
    /// or a clean `connection_pool` error if no slot can be opened
    /// within [`pool_create_timeout`](Self::pool_create_timeout).
    ///
    /// The trade-off is one round-trip per checkout. Combine with a
    /// larger [`max_pool_size`](Self::max_pool_size) if the extra
    /// latency starts queueing requests. Independent of the background
    /// sweep — most deployments want one or the other, but enabling
    /// both is safe.
    ///
    /// Defaults to `false`.
    pub fn pool_pre_ping(&mut self, pre_ping: bool) -> &mut Self {
        self.pool.pre_ping = pre_ping;
        self
    }

    /// Evict any pooled connection older than this duration when the
    /// pool considers reusing it. Useful when an idle timeout on the
    /// server, a load balancer, or a NAT in front of the database
    /// silently closes long-lived sockets — capping the lifetime
    /// bounds how long a connection can survive past such a close.
    ///
    /// The check runs in `recycle` (when the pool considers handing
    /// the connection back out), not in the background. A query that
    /// holds a connection past the cap is allowed to finish; the
    /// connection is evicted on its next return.
    ///
    /// Recommended for any deployment that talks to a remote database:
    /// pick a duration shorter than every idle timeout in the path
    /// (server, load balancer, NAT). 30 minutes works for most clouds.
    ///
    /// Defaults to `None` (no cap).
    pub fn pool_max_connection_lifetime(&mut self, lifetime: Option<Duration>) -> &mut Self {
        self.pool.max_connection_lifetime = lifetime;
        self
    }

    /// Evict any pooled connection that has been sitting unused for
    /// longer than this duration. Complements
    /// [`pool_max_connection_lifetime`](Self::pool_max_connection_lifetime):
    /// the idle cap targets connections specifically held idle past a
    /// known timeout, the lifetime cap targets all connections
    /// regardless of recent use.
    ///
    /// Checked in `recycle`, not in the background.
    ///
    /// Defaults to `None` (no cap).
    pub fn pool_max_connection_idle_time(&mut self, idle: Option<Duration>) -> &mut Self {
        self.pool.max_connection_idle_time = idle;
        self
    }

    /// Build and return the app-level schema from the registered models
    /// without opening a database connection.
    ///
    /// This is useful for tooling that needs the schema without a running
    /// database (e.g., migration generators).
    pub fn build_app_schema(&self) -> Result<app::Schema> {
        app::Schema::from_macro(self.models.clone())
    }

    /// Open a database connection using a URL string.
    ///
    /// The URL scheme selects the driver (`sqlite://`, `postgresql://`,
    /// `mysql://`, `dynamodb://`). The corresponding feature flag must be
    /// enabled.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL cannot be parsed, the scheme is
    /// unsupported or its feature flag is not enabled, or the initial
    /// connection fails.
    ///
    /// # Examples
    ///
    /// ```
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// let db = toasty::Db::builder()
    ///     .models(toasty::models!(User))
    ///     .connect("sqlite://memory")
    ///     .await
    ///     .unwrap();
    /// # });
    /// ```
    pub async fn connect(&mut self, url: &str) -> Result<Db> {
        let con = Connect::new(url).await?;
        self.build(con).await
    }

    /// Build a [`Db`] from an already-constructed driver instance.
    ///
    /// Use this instead of [`connect`](Self::connect) when you need to
    /// configure the driver yourself (e.g., an in-memory SQLite database for
    /// tests).
    ///
    /// # Errors
    ///
    /// Returns an error if the driver reports invalid capabilities or the
    /// initial connection cannot be acquired.
    ///
    /// # Examples
    ///
    /// ```
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// # #[derive(Debug, toasty::Model)]
    /// # struct User {
    /// #     #[key]
    /// #     id: i64,
    /// #     name: String,
    /// # }
    /// let driver = toasty_driver_sqlite::Sqlite::in_memory();
    /// let db = toasty::Db::builder()
    ///     .models(toasty::models!(User))
    ///     .build(driver)
    ///     .await
    ///     .unwrap();
    /// # });
    /// ```
    pub async fn build(&mut self, driver: impl Driver) -> Result<Db> {
        tracing::info!(models = self.models.len(), "building database schema");
        let capability = driver.capability();
        capability.validate()?;
        let schema = self.core.build(self.build_app_schema()?, capability)?;

        tracing::info!(tables = schema.db.tables.len(), "schema built successfully");

        let engine = Engine::new(Arc::new(schema), capability);
        let pool = Pool::new(driver, engine.clone(), self.pool.clone())?;

        let shared = Arc::new(Shared { engine, pool });

        // see if we're able to acquire a valid connection
        let conn = shared.pool.get(shared.clone()).await?;
        std::mem::drop(conn);

        tracing::info!("database ready");
        Ok(Db { shared })
    }
}

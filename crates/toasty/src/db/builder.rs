use crate::{
    Db, Result,
    db::{Connect, Pool, Shared},
    engine::Engine,
};

use toasty_core::{
    driver::Driver,
    schema::{
        self,
        app::{self, ModelSet},
    },
};

use std::sync::Arc;

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
}

impl Builder {
    /// Set the models whose schemas will be included when the database is
    /// built.
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
        let pool = Pool::new(driver, engine.clone())?;

        let shared = Arc::new(Shared { engine, pool });

        // see if we're able to acquire a valid connection
        let conn = shared.pool.get(shared.clone()).await?;
        std::mem::drop(conn);

        tracing::info!("database ready");
        Ok(Db { shared })
    }
}

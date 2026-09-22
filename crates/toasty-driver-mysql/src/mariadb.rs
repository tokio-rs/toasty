use super::MySQL;
use async_trait::async_trait;
use std::borrow::Cow;
use toasty_core::{
    Result,
    driver::{Capability, ConnectContext, Connection, Driver},
    schema::{db::Migration, diff},
};

/// A MariaDB 11.8+ [`Driver`] that connects through SQLx.
///
/// Uses MariaDB's native `UUID` type and `INSERT ... RETURNING`.
///
/// # Examples
///
/// ```
/// use toasty_driver_mysql::MariaDB;
///
/// let driver = MariaDB::new("mariadb://localhost/mydb").unwrap();
/// ```
#[derive(Debug)]
pub struct MariaDB {
    inner: MySQL,
}

impl MariaDB {
    /// Creates a MariaDB driver without connecting to the server.
    ///
    /// The URL must use the `mariadb` scheme and include a database path,
    /// such as `mariadb://user:pass@host:3306/dbname`. The server must run
    /// MariaDB 11.8 or later.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL is malformed or uses another scheme.
    pub fn new(url: impl Into<String>) -> Result<Self> {
        Ok(Self {
            inner: MySQL::with_capability(url, "mariadb", &Capability::MARIADB)?,
        })
    }
}

#[async_trait]
impl Driver for MariaDB {
    fn url(&self) -> Cow<'_, str> {
        self.inner.url()
    }

    fn capability(&self) -> &'static Capability {
        &Capability::MARIADB
    }

    async fn connect(&self, cx: &ConnectContext) -> Result<Box<dyn Connection>> {
        self.inner.connect(cx).await
    }

    fn generate_migration(&self, schema_diff: &diff::Schema<'_>) -> Migration {
        self.inner.generate_migration(schema_diff)
    }

    async fn reset_db(&self) -> Result<()> {
        self.inner.reset_db().await
    }
}

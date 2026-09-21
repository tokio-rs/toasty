#![warn(missing_docs)]

//! Toasty driver for [MariaDB](https://mariadb.org/) 11.8 and later.
//!
//! Uses the MySQL wire protocol with MariaDB's native `UUID` type and
//! `INSERT ... RETURNING`. Enable Toasty's `mariadb` feature to connect
//! using a `mariadb://` URL.
//!
//! # Examples
//!
//! ```
//! use toasty_driver_mariadb::MariaDb;
//!
//! let driver = MariaDb::new("mariadb://localhost/mydb").unwrap();
//! ```

use async_trait::async_trait;
use std::borrow::Cow;
use toasty_core::{
    Result,
    driver::{Capability, ConnectContext, Connection, Driver},
    schema::{db::Migration, diff},
};
use toasty_driver_mysql::MySqlProtocol;

/// A MariaDB [`Driver`] that connects through SQLx.
#[derive(Debug)]
pub struct MariaDb {
    inner: MySqlProtocol,
}

impl MariaDb {
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
            inner: MySqlProtocol::new(url, "mariadb", &Capability::MARIADB)?,
        })
    }
}

#[async_trait]
impl Driver for MariaDb {
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

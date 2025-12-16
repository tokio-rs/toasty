use crate::Result;

pub use toasty_core::{
    driver::{operation::Operation, Capability, Driver, Response},
    schema::db::Schema,
};

use std::sync::Arc;
use url::Url;

#[derive(Debug)]
pub(crate) enum Flavor {
    #[cfg(feature = "dynamodb")]
    DynamoDb(toasty_driver_dynamodb::DynamoDb),

    #[cfg(feature = "mysql")]
    MySQL(toasty_driver_mysql::MySQL),

    #[cfg(feature = "postgresql")]
    PostgreSQL(toasty_driver_postgresql::PostgreSQL),

    #[cfg(feature = "sqlite")]
    Sqlite(toasty_driver_sqlite::Sqlite),
}

/// A connection to a database, wrapping the specific driver implementation.
#[derive(Debug)]
pub struct Connection(pub(crate) Flavor);

impl Connection {
    pub async fn connect(url: &str) -> Result<Self> {
        let url = Url::parse(url)?;

        match url.scheme() {
            "dynamodb" => Self::connect_dynamodb(&url).await,
            "mysql" => Self::connect_mysql(&url).await,
            "postgresql" => Self::connect_postgresql(&url).await,
            "sqlite" => Self::connect_sqlite(&url),
            scheme => Err(anyhow::anyhow!(
                "unsupported database; schema={scheme}; url={url}"
            )),
        }
    }

    #[cfg(feature = "dynamodb")]
    async fn connect_dynamodb(url: &Url) -> Result<Connection> {
        let driver = toasty_driver_dynamodb::DynamoDb::connect(url.as_str()).await?;
        Ok(Connection(Flavor::DynamoDb(driver)))
    }

    #[cfg(not(feature = "dynamodb"))]
    async fn connect_dynamodb(_url: &Url) -> Result<Self> {
        Err(anyhow::anyhow!("`dynamodb` feature not enabled"))
    }

    #[cfg(feature = "mysql")]
    async fn connect_mysql(url: &Url) -> Result<Connection> {
        let driver = toasty_driver_mysql::MySQL::connect(url.as_str()).await?;
        Ok(Connection(Flavor::MySQL(driver)))
    }

    #[cfg(not(feature = "mysql"))]
    async fn connect_mysql(_url: &Url) -> Result<Self> {
        Err(anyhow::anyhow!("`mysql` feature not enabled"))
    }

    #[cfg(feature = "postgresql")]
    async fn connect_postgresql(url: &Url) -> Result<Connection> {
        let driver = toasty_driver_postgresql::PostgreSQL::connect(url.as_str()).await?;
        Ok(Connection(Flavor::PostgreSQL(driver)))
    }

    #[cfg(not(feature = "postgresql"))]
    async fn connect_postgresql(_url: &Url) -> Result<Self> {
        Err(anyhow::anyhow!("`postgresql` feature not enabled"))
    }

    #[cfg(feature = "sqlite")]
    fn connect_sqlite(url: &Url) -> Result<Connection> {
        let driver = toasty_driver_sqlite::Sqlite::connect(url.as_str())?;
        Ok(Connection(Flavor::Sqlite(driver)))
    }

    #[cfg(not(feature = "sqlite"))]
    fn connect_sqlite(_url: &Url) -> Result<Connection> {
        Err(anyhow::anyhow!("`sqlite` feature not enabled"))
    }
}

macro_rules! match_db {
    ($self:expr, $driver:pat => $e:expr) => {
        match $self.0 {
            #[cfg(feature = "dynamodb")]
            Flavor::DynamoDb($driver) => $e,

            #[cfg(feature = "mysql")]
            Flavor::MySQL($driver) => $e,

            #[cfg(feature = "postgresql")]
            Flavor::PostgreSQL($driver) => $e,

            #[cfg(feature = "sqlite")]
            Flavor::Sqlite($driver) => $e,
        }
    };
}

#[toasty_core::async_trait]
impl Driver for Connection {
    fn capability(&self) -> &'static Capability {
        match_db!(self, ref driver => driver.capability())
    }

    async fn exec(&self, schema: &Arc<Schema>, op: Operation) -> Result<Response> {
        #[allow(unused_variables)]
        let schema = schema;
        #[allow(unused_variables)]
        let op = op;

        match_db!(self, ref driver => driver.exec(schema, op).await)
    }

    async fn reset_db(&self, schema: &Schema) -> Result<()> {
        #[allow(unused_variables)]
        let schema = schema;
        match_db!(self, ref driver => driver.reset_db(schema).await)
    }
}

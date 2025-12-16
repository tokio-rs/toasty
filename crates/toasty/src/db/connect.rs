use crate::Result;

use toasty_core::driver::Driver;
pub use toasty_core::driver::{operation::Operation, Capability, Connection, Response};

use url::Url;

/// A connection to a database, wrapping the specific driver implementation.
#[derive(Debug)]
pub struct Connect {
    url: Url,
}

impl Connect {
    pub fn new(url: &str) -> Result<Self> {
        let url = Url::parse(url)?;
        Ok(Self { url })
    }
}

impl Driver for Connect {
    async fn connect(&self) -> Result<Box<dyn Connection>> {
        match self.url.scheme() {
            "dynamodb" => toasty_driver_dynamodb::DynamoDb::connect(url.as_str()),
            #[cfg(not(feature = "dynamodb"))]
            "dynamodb" => anyhow::bail!("`dynamodb` feature not enabled"),

            "mysql" => connect_mysql(&self.url).await,
            "postgresql" => connect_postgresql(&self.url).await,
            "sqlite" => connect_sqlite(&self.url),
            scheme => Err(anyhow::anyhow!(
                "unsupported database; schema={scheme}; url={url}"
            )),
        }
    }
}

#[cfg(feature = "dynamodb")]
async fn connect_dynamodb(url: &Url) -> Result<Box<dyn Connection>> {
    let driver = toasty_driver_dynamodb::DynamoDb::connect(url.as_str()).await?;
    Ok(Connection(Flavor::DynamoDb(driver)))
}

#[cfg(not(feature = "dynamodb"))]
async fn connect_dynamodb(_url: &Url) -> Result<Box<dyn Connection>> {
    Err(anyhow::anyhow!("`dynamodb` feature not enabled"))
}

#[cfg(feature = "mysql")]
async fn connect_mysql(url: &Url) -> Result<Box<dyn Connection>> {
    let driver = toasty_driver_mysql::MySQL::connect(url.as_str()).await?;
    Ok(Connection(Flavor::MySQL(driver)))
}

#[cfg(not(feature = "mysql"))]
async fn connect_mysql(_url: &Url) -> Result<Box<dyn Connection>> {
    Err(anyhow::anyhow!("`mysql` feature not enabled"))
}

#[cfg(feature = "postgresql")]
async fn connect_postgresql(url: &Url) -> Result<Box<dyn Connection>> {
    toasty_driver_postgresql::PostgreSQL::connect(url.as_str())
}

#[cfg(not(feature = "postgresql"))]
async fn connect_postgresql(_url: &Url) -> Result<Box<dyn Connection>> {
    Err(anyhow::anyhow!("`postgresql` feature not enabled"))
}

#[cfg(feature = "sqlite")]
fn connect_sqlite(url: &Url) -> Result<Box<dyn Connection>> {
    toasty_driver_sqlite::Connection::connect(url.as_str())
}

#[cfg(not(feature = "sqlite"))]
fn connect_sqlite(_url: &Url) -> Result<Box<dyn Connection>> {
    Err(anyhow::anyhow!("`sqlite` feature not enabled"))
}

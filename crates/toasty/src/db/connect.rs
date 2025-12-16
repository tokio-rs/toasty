use crate::Result;

pub use toasty_core::driver::{operation::Operation, Capability, Connection, Response};
use toasty_core::{async_trait, driver::Driver};

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

#[async_trait]
impl Driver for Connect {
    async fn connect(&self) -> Result<Box<dyn Connection>> {
        match self.url.scheme() {
            #[cfg(feature = "dynamodb")]
            "dynamodb" => toasty_driver_dynamodb::DynamoDb::connect(url.as_str()),
            #[cfg(not(feature = "dynamodb"))]
            "dynamodb" => anyhow::bail!("`dynamodb` feature not enabled"),

            #[cfg(feature = "mysql")]
            "mysql" => {
                toasty_driver_mysql::MySQL::new(self.url.to_string())
                    .connect()
                    .await
            }
            #[cfg(not(feature = "mysql"))]
            "mysql" => anyhow::bail!("`mysql` feature not enabled"),

            #[cfg(feature = "postgresql")]
            "postgresql" => {
                toasty_driver_postgresql::PostgreSQL::new(self.url.to_string())
                    .connect()
                    .await
            }
            #[cfg(not(feature = "postgresql"))]
            "postgresql" => anyhow::bail!("`postgresql` feature not enabled"),

            #[cfg(feature = "sqlite")]
            "sqlite" => {
                toasty_driver_sqlite::Sqlite::Url(self.url.to_string())
                    .connect()
                    .await
            }
            #[cfg(not(feature = "sqlite"))]
            "sqlite" => anyhow::bail!("`sqlite` feature not enabled"),

            scheme => Err(anyhow::anyhow!(
                "unsupported database; schema={scheme}; url={}",
                self.url
            )),
        }
    }
}

use crate::Result;

pub use toasty_core::driver::{operation::Operation, Capability, Connection, Response};
use toasty_core::{async_trait, driver::Driver};

use url::Url;

/// A connection to a database, wrapping the specific driver implementation.
pub struct Connect {
    driver: Box<dyn Driver>,
}

impl std::fmt::Debug for Connect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Connect")
            .field("driver", &self.driver)
            .finish()
    }
}

impl Connect {
    pub fn new(url: &str) -> Result<Self> {
        #![cfg_attr(
            not(any(
                feature = "dynamodb",
                feature = "mysql",
                feature = "postgresql",
                feature = "sqlite"
            )),
            allow(unused_variables, unreachable_code)
        )]

        let url = Url::parse(url).map_err(toasty_core::Error::driver_operation_failed)?;

        let driver: Box<dyn Driver> = match url.scheme() {
            #[cfg(feature = "dynamodb")]
            "dynamodb" => Box::new(toasty_driver_dynamodb::DynamoDb::new(url.to_string())),
            #[cfg(not(feature = "dynamodb"))]
            "dynamodb" => {
                return Err(toasty_core::Error::unsupported_feature(
                    "`dynamodb` feature not enabled",
                ))
            }

            #[cfg(feature = "mysql")]
            "mysql" => Box::new(toasty_driver_mysql::MySQL::new(url.to_string())?),
            #[cfg(not(feature = "mysql"))]
            "mysql" => {
                return Err(toasty_core::Error::unsupported_feature(
                    "`mysql` feature not enabled",
                ))
            }

            #[cfg(feature = "postgresql")]
            "postgresql" => Box::new(toasty_driver_postgresql::PostgreSQL::new(url)?),
            #[cfg(not(feature = "postgresql"))]
            "postgresql" => {
                return Err(toasty_core::Error::unsupported_feature(
                    "`postgresql` feature not enabled",
                ))
            }

            #[cfg(feature = "sqlite")]
            "sqlite" => Box::new(toasty_driver_sqlite::Sqlite::new(url)?),
            #[cfg(not(feature = "sqlite"))]
            "sqlite" => {
                return Err(toasty_core::Error::unsupported_feature(
                    "`sqlite` feature not enabled",
                ))
            }

            scheme => {
                return Err(toasty_core::Error::unsupported_feature(format!(
                    "unsupported database scheme `{scheme}`"
                )))
            }
        };

        Ok(Self { driver })
    }
}

#[async_trait]
impl Driver for Connect {
    async fn connect(&self) -> Result<Box<dyn Connection>> {
        self.driver.connect().await
    }
}

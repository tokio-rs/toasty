use crate::Result;

pub use toasty_core::{
    driver::{
        capability,
        operation::{self, Operation},
        Capability, Driver, Response, Rows,
    },
    schema::db::Schema,
};

use std::sync::Arc;
use url::Url;

#[derive(Debug)]
pub(crate) enum Connection {
    #[cfg(feature = "sqlite")]
    Sqlite(toasty_sqlite::Sqlite),
}

impl Connection {
    pub(crate) async fn connect(url: &str) -> Result<Connection> {
        let url = Url::parse(url)?;

        match url.scheme() {
            "sqlite" => Self::from_sqlite(&url),
            _ => return Err(anyhow::anyhow!("unsupported database; url={url}")),
        }
    }

    #[cfg(feature = "sqlite")]
    fn from_sqlite(url: &Url) -> Result<Connection> {
        let driver = toasty_sqlite::Sqlite::from_url(url.as_str())?;
        Ok(Connection::Sqlite(driver))
    }

    #[cfg(not(feature = "sqlite"))]
    fn from_sqlite(_url: &Url) -> Result<Connection> {
        Err(anyhow::anyhow!("`sqlite` feature not enabled"))
    }
}

macro_rules! match_db {
    ($self:expr, $driver:pat => $e:expr) => {
        match *$self {
            #[cfg(feature = "sqlite")]
            Connection::Sqlite($driver) => $e,
        }
    };
}

#[toasty_core::async_trait]
impl Driver for Connection {
    fn capability(&self) -> &Capability {
        match_db!(self, ref driver => driver.capability())
    }

    async fn register_schema(&mut self, schema: &Schema) -> Result<()> {
        #[allow(unused_variables)]
        let schema = schema;
        match_db!(self, ref mut driver => driver.register_schema(schema).await)
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

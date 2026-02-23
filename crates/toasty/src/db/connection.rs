use std::sync::Arc;

use toasty_core::{
    driver::{Operation, Response},
    schema::db::Schema,
};

use crate::db::{Pool, PoolConnection};

#[derive(Debug)]
pub(crate) enum ConnectionType {
    Pool(Pool),
    Transaction(PoolConnection),
}

pub(crate) enum SingleConnection<'a> {
    Pooled(PoolConnection),
    Transaction(&'a mut PoolConnection),
}

impl ConnectionType {
    pub fn in_transaction(&self) -> bool {
        matches!(self, ConnectionType::Transaction(_))
    }

    pub async fn get(&mut self) -> crate::Result<SingleConnection<'_>> {
        match self {
            ConnectionType::Pool(pool) => pool.get().await.map(SingleConnection::Pooled),
            ConnectionType::Transaction(conn) => Ok(SingleConnection::Transaction(conn)),
        }
    }

    pub async fn push_schema(&mut self, schema: &Schema) -> crate::Result<()> {
        match self {
            ConnectionType::Pool(pool) => pool.get().await?.push_schema(schema).await,
            ConnectionType::Transaction(conn) => conn.push_schema(schema).await,
        }
    }
}

impl SingleConnection<'_> {
    pub async fn exec(&mut self, schema: &Arc<Schema>, plan: Operation) -> crate::Result<Response> {
        match self {
            SingleConnection::Pooled(pool_connection) => pool_connection.exec(schema, plan).await,
            SingleConnection::Transaction(mutex_guard) => mutex_guard.exec(schema, plan).await,
        }
    }
}

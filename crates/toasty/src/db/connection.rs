use std::sync::Arc;

use toasty_core::{
    driver::{Operation, Response},
    schema::db::Schema,
};

use crate::db::{Pool, PoolConnection};

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
}

impl SingleConnection<'_> {
    pub async fn exec(&mut self, schema: &Arc<Schema>, plan: Operation) -> crate::Result<Response> {
        match self {
            SingleConnection::Pooled(pooled_conn) => pooled_conn.exec(schema, plan).await,
            SingleConnection::Transaction(conn) => conn.exec(schema, plan).await,
        }
    }
}

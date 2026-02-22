use std::sync::Arc;

use toasty_core::{
    driver::{Operation, Response},
    schema::db::Schema,
};
use tokio::sync::{Mutex, MutexGuard};

use crate::db::{Pool, PoolConnection};

#[derive(Debug)]
pub(crate) enum ConnectionType {
    Pool(Pool),
    Transaction(Arc<Mutex<PoolConnection>>),
}

pub(crate) enum SingleConnection<'a> {
    Pooled(PoolConnection),
    Transaction(MutexGuard<'a, PoolConnection>),
}

impl ConnectionType {
    pub async fn get(&self) -> crate::Result<SingleConnection<'_>> {
        match self {
            ConnectionType::Pool(pool) => pool.get().await.map(SingleConnection::Pooled),
            ConnectionType::Transaction(mutex) => {
                Ok(SingleConnection::Transaction(mutex.lock().await))
            }
        }
    }

    pub async fn push_schema(&self, schema: &Schema) -> crate::Result<()> {
        match self {
            ConnectionType::Pool(pool) => pool.get().await?.push_schema(schema).await,
            ConnectionType::Transaction(mutex) => mutex.lock().await.push_schema(schema).await,
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

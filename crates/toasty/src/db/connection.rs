use std::sync::Arc;

use toasty_core::{
    driver::{Operation, Response},
    schema::db::Schema,
};

use crate::db::{Pool, PoolConnection};

pub(crate) enum ConnectionSource {
    Pool(Pool),
    Transaction(PoolConnection),
}

pub(crate) enum ConnHandle<'a> {
    Pooled(PoolConnection),
    Transaction(&'a mut PoolConnection),
}

impl ConnectionSource {
    pub fn in_transaction(&self) -> bool {
        matches!(self, ConnectionSource::Transaction(_))
    }

    pub async fn get(&mut self) -> crate::Result<ConnHandle<'_>> {
        match self {
            ConnectionSource::Pool(pool) => pool.get().await.map(ConnHandle::Pooled),
            ConnectionSource::Transaction(conn) => Ok(ConnHandle::Transaction(conn)),
        }
    }
}

impl ConnHandle<'_> {
    pub async fn exec(&mut self, schema: &Arc<Schema>, plan: Operation) -> crate::Result<Response> {
        match self {
            ConnHandle::Pooled(pooled_conn) => pooled_conn.exec(schema, plan).await,
            ConnHandle::Transaction(conn) => conn.exec(schema, plan).await,
        }
    }
}

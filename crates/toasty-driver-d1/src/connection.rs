use std::{fmt, sync::Arc};

use async_trait::async_trait;
use toasty_core::{
    Schema,
    driver::{ExecResponse, QueryLogConfig, operation::Operation},
    schema::db::{AppliedMigration, Migration},
};
use worker::{D1Database, send::SendWrapper};

use crate::error;

pub(crate) struct Connection {
    #[allow(dead_code)]
    database: SendWrapper<D1Database>,
    #[allow(dead_code)]
    query_log: QueryLogConfig,
}

impl Connection {
    pub(crate) fn new(database: SendWrapper<D1Database>, query_log: QueryLogConfig) -> Self {
        Self {
            database,
            query_log,
        }
    }
}

impl fmt::Debug for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection").finish_non_exhaustive()
    }
}

#[async_trait]
impl toasty_core::Connection for Connection {
    async fn exec(
        &mut self,
        _schema: &Arc<Schema>,
        operation: Operation,
    ) -> toasty_core::Result<ExecResponse> {
        match operation {
            Operation::QuerySql(_) => Err(error::unsupported("query_sql")),
            Operation::RawSql(_) => Err(error::unsupported("raw_sql")),
            Operation::Transaction(_) => Err(error::unsupported("transaction")),
            operation => Err(error::unsupported(operation.name())),
        }
    }

    async fn push_schema(&mut self, _schema: &Schema) -> toasty_core::Result<()> {
        Err(error::unsupported("push schema"))
    }

    async fn applied_migrations(&mut self) -> toasty_core::Result<Vec<AppliedMigration>> {
        Err(error::unsupported("read applied migrations"))
    }

    async fn apply_migration(
        &mut self,
        _id: u64,
        _name: &str,
        _migration: &Migration,
    ) -> toasty_core::Result<()> {
        Err(error::unsupported("apply migration"))
    }
}

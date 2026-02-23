mod builder;
mod connect;
mod connection;
mod pool;
mod transaction;

use std::{future::Future, sync::Arc};

pub use builder::Builder;
pub use connect::*;
pub(crate) use connection::{ConnectionType, SingleConnection};
pub use pool::*;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};
pub use transaction::TransactionBuilder;

use crate::{engine::Engine, stmt, Cursor, Model, Result, Statement};

use toasty_core::{
    driver::Driver,
    stmt::{Value, ValueStream},
    Schema,
};

#[derive(Debug)]
pub struct Db {
    in_transaction: bool,
    pool: Pool,

    pub(crate) schema: Arc<Schema>,

    // pub(crate) engine: Engine,
    /// Handle to send statements to be executed
    pub(crate) in_tx: mpsc::UnboundedSender<(
        toasty_core::stmt::Statement,
        oneshot::Sender<Result<ValueStream>>,
    )>,

    /// Handle to task driving the query engine
    pub(crate) join_handle: JoinHandle<()>,
}

impl Db {
    pub(crate) fn new(pool: Pool, schema: Arc<Schema>, mut connection: ConnectionType) -> Self {
        let capabilities = pool.capability();
        let in_transaction = connection.in_transaction();
        let pool2 = pool.clone();

        let mut engine = Engine::new(schema.clone(), capabilities);

        let (in_tx, mut in_rx) = tokio::sync::mpsc::unbounded_channel::<(
            toasty_core::stmt::Statement,
            oneshot::Sender<crate::Result<ValueStream>>,
        )>();

        let join_handle = tokio::spawn(async move {
            loop {
                let (stmt, tx) = in_rx.recv().await.unwrap();
                let conn = match connection.get().await {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = tx.send(Err(e));
                        continue;
                    }
                };
                match engine.exec(stmt, conn).await {
                    Ok(mut value_stream) => {
                        let (row_tx, mut row_rx) =
                            tokio::sync::mpsc::unbounded_channel::<crate::Result<Value>>();

                        let _ = tx.send(Ok(ValueStream::from_stream(async_stream::stream! {
                            while let Some(res) = row_rx.recv().await {
                                yield res
                            }
                        })));

                        while let Some(res) = value_stream.next().await {
                            let _ = row_tx.send(res);
                        }
                    }
                    Err(err) => {
                        let _ = tx.send(Err(err));
                    }
                }
            }
        });

        Db {
            in_transaction,
            pool: pool2,
            schema,
            in_tx,
            join_handle,
        }
    }

    pub fn builder() -> Builder {
        Builder::default()
    }

    /// Execute a query, returning all matching records
    pub async fn all<M: Model>(&self, query: stmt::Select<M>) -> Result<Cursor<M>> {
        let records = self.exec(query.into()).await?;
        Ok(Cursor::new(self.schema.clone(), records))
    }

    pub async fn first<M: Model>(&self, query: stmt::Select<M>) -> Result<Option<M>> {
        let mut res = self.all(query).await?;
        match res.next().await {
            Some(Ok(value)) => Ok(Some(value)),
            Some(Err(err)) => Err(err),
            None => Ok(None),
        }
    }

    pub async fn get<M: Model>(&self, query: stmt::Select<M>) -> Result<M> {
        let mut res = self.all(query).await?;

        match res.next().await {
            Some(Ok(value)) => Ok(value),
            Some(Err(err)) => Err(err),
            None => Err(toasty_core::Error::record_not_found(
                "query returned no results",
            )),
        }
    }

    pub async fn delete<M: Model>(&self, query: stmt::Select<M>) -> Result<()> {
        self.exec(query.delete()).await?;
        Ok(())
    }

    /// Execute a statement
    pub fn exec<M: Model>(
        &self,
        statement: Statement<M>,
    ) -> impl Future<Output = Result<ValueStream>> {
        let (tx, rx) = oneshot::channel();

        // Send the statement to the execution engine
        self.in_tx.send((statement.untyped, tx)).unwrap();

        async {
            // Return the typed result
            rx.await.unwrap()
        }
    }

    /// Execute a statement, assume only one record is returned
    #[doc(hidden)]
    pub async fn exec_one<M: Model>(&self, statement: Statement<M>) -> Result<stmt::Value> {
        let mut res = self.exec(statement).await?;
        let Some(ret) = res.next().await else {
            return Err(toasty_core::Error::record_not_found(
                "statement returned no results",
            ));
        };
        let next = res.next().await;
        let None = next else {
            return Err(toasty_core::Error::invalid_record_count(
                "expected 1 record, found multiple",
            ));
        };

        ret
    }

    /// Execute model creation
    ///
    /// Used by generated code
    #[doc(hidden)]
    pub async fn exec_insert_one<M: Model>(&self, mut stmt: stmt::Insert<M>) -> Result<M> {
        // TODO: HAX
        stmt.untyped.source.single = false;

        // Execute the statement and return the result
        let records = self.exec(stmt.into()).await?;
        let mut cursor = Cursor::new(self.schema.clone(), records);

        cursor.next().await.unwrap()
    }

    /// Creates tables and indices defined in the schema on the database.
    pub async fn push_schema(&self) -> Result<()> {
        todo!();
        // self.engine.connection.push_schema(&self.schema.db).await
    }

    /// Drops the entire database and recreates an empty one without applying migrations.
    pub async fn reset_db(&self) -> Result<()> {
        self.driver().reset_db().await
    }

    pub fn driver(&self) -> &dyn Driver {
        self.pool.driver()
    }

    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    pub fn capability(&self) -> &Capability {
        self.pool.capability()
    }
}

impl Drop for Db {
    fn drop(&mut self) {
        if !self.in_transaction {
            // TODO: make this less aggressive
            self.join_handle.abort();
        }
    }
}

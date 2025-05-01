mod builder;
pub use builder::Builder;

use crate::{driver::Driver, engine, stmt, Cursor, Model, Result, Statement};

use toasty_core::{stmt::ValueStream, Schema};

use std::sync::Arc;

#[derive(Debug)]
pub struct Db {
    /// Handle to the underlying database driver.
    pub(crate) driver: Arc<dyn Driver>,

    /// Schema being managed by this DB instance.
    pub(crate) schema: Arc<Schema>,
}

impl Db {
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
            None => anyhow::bail!("failed to find record"),
        }
    }

    pub async fn delete<M: Model>(&self, query: stmt::Select<M>) -> Result<()> {
        self.exec(query.delete()).await?;
        Ok(())
    }

    /// Execute a statement
    pub async fn exec<M: Model>(&self, statement: Statement<M>) -> Result<ValueStream> {
        // Create a plan to execute the statement
        let mut res = engine::exec(self, statement.untyped).await?;

        // If the execution is lazy, force it to begin.
        res.tap().await?;

        // Return the typed result
        Ok(res)
    }

    /// Execute a statement, assume only one record is returned
    #[doc(hidden)]
    pub async fn exec_one<M: Model>(&self, statement: Statement<M>) -> Result<stmt::Value> {
        let mut res = self.exec(statement).await?;
        let Some(ret) = res.next().await else {
            anyhow::bail!("empty result set")
        };
        let None = res.next().await else {
            anyhow::bail!("more than one record")
        };

        ret
    }

    /// Execute model creation
    ///
    /// Used by generated code
    #[doc(hidden)]
    pub async fn exec_insert_one<M: Model>(&self, stmt: stmt::Insert<M>) -> Result<M> {
        // Execute the statement and return the result
        let records = self.exec(stmt.into()).await?;
        let mut cursor = Cursor::new(self.schema.clone(), records);

        cursor.next().await.unwrap()
    }

    /// TODO: remove
    pub async fn reset_db(&self) -> Result<()> {
        self.driver.reset_db(&self.schema.db).await
    }

    pub fn schema(&self) -> &Schema {
        &self.schema
    }
}

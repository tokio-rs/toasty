use crate::{driver::Driver, engine, stmt, Cursor, Model, Result, Statement};

use toasty_core::{stmt::ValueStream, Schema};

use std::sync::Arc;

#[derive(Debug)]
pub struct Db {
    /// Handle to the underlying database driver.
    pub(crate) driver: Arc<dyn Driver>,

    /// Schema being managed by this DB instance.
    pub(crate) schema: Schema,
}

impl Db {
    pub async fn new(schema: Schema, mut driver: impl Driver) -> Db {
        driver.register_schema(&schema).await.unwrap();

        Db {
            driver: Arc::new(driver),
            schema: schema,
        }
    }

    /// Execute a query, returning all matching records
    pub async fn all<'stmt, M: Model>(
        &self,
        query: stmt::Select<'stmt, M>,
    ) -> Result<Cursor<'stmt, M>> {
        let records = self.exec(query.into()).await?;
        Ok(Cursor::new(self.schema.clone(), records))
    }

    pub async fn first<'stmt, M: Model>(&self, query: stmt::Select<'stmt, M>) -> Result<Option<M>> {
        let mut res = self.all(query).await?;
        match res.next().await {
            Some(Ok(value)) => Ok(Some(value)),
            Some(Err(err)) => Err(err),
            None => Ok(None),
        }
    }

    pub async fn get<'stmt, M: Model>(&self, query: stmt::Select<'stmt, M>) -> Result<M> {
        let mut res = self.all(query).await?;

        match res.next().await {
            Some(Ok(value)) => Ok(value),
            Some(Err(err)) => Err(err),
            None => anyhow::bail!("failed to find record"),
        }
    }

    pub async fn delete<'stmt, Q>(&self, query: Q) -> Result<()>
    where
        Q: stmt::IntoSelect<'stmt>,
    {
        self.exec(query.into_select().delete()).await?;
        Ok(())
    }

    /// Execute a statement
    pub async fn exec<'stmt, M: Model>(
        &self,
        statement: Statement<'stmt, M>,
    ) -> Result<ValueStream<'stmt>> {
        // Create a plan to execute the statement
        let mut res = engine::exec(self, statement.untyped).await?;

        // If the execution is lazy, force it to begin.
        res.tap().await?;

        // Return the typed result
        Ok(res)
    }

    /// Execute model creation
    ///
    /// Used by generated code
    #[doc(hidden)]
    pub async fn exec_insert_one<'stmt, M: Model>(
        &self,
        stmt: stmt::Insert<'stmt, M>,
    ) -> Result<M> {
        // TODO: get rid of this assertion and move to verify
        let toasty_core::stmt::Expr::Record(expr_record) = &stmt.untyped.values else {
            todo!()
        };
        assert!(!expr_record.is_empty());

        // Execute the statement and return the result
        let records = self.exec(stmt.into()).await?;
        let mut cursor = Cursor::new(self.schema.clone(), records);

        cursor.next().await.unwrap()
    }

    /// TODO: remove
    pub async fn reset_db(&self) -> Result<()> {
        self.driver.reset_db(&self.schema).await
    }
}

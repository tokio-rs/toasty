use crate::{driver::Driver, engine, stmt, Cursor, Model, Result, Statement};

use toasty_core::{stmt::ValueStream, Schema};
use tracing::debug;

#[derive(Debug)]
pub struct Db {
    /// Handle to the underlying database driver.
    pub(crate) driver: Box<dyn Driver>,

    /// Schema being managed by this DB instance.
    pub(crate) schema: Schema,
}

impl Db {
    pub async fn new(schema: Schema, driver: impl Driver) -> Db {
        let mut driver = Box::new(driver);
        driver.register_schema(&schema).await.unwrap();

        Db { driver, schema }
    }

    /// Execute a query, returning all matching records
    pub async fn all<'a, Q>(
        &'a self,
        query: Q,
    ) -> Result<Cursor<'a, <Q as stmt::IntoSelect<'a>>::Model>>
    where
        Q: stmt::IntoSelect<'a>,
    {
        let select = query.into_select();
        let records = self.exec(select.into()).await?;
        Ok(Cursor::new(&self.schema, records))
    }

    pub async fn first<'a, Q>(
        &'a self,
        query: Q,
    ) -> Result<Option<<Q as stmt::IntoSelect<'a>>::Model>>
    where
        Q: stmt::IntoSelect<'a>,
    {
        let mut res = self.all(query).await?;
        match res.next().await {
            Some(Ok(value)) => Ok(Some(value)),
            Some(Err(err)) => Err(err),
            None => Ok(None),
        }
    }

    pub async fn get<'a, Q>(&'a self, query: Q) -> Result<<Q as stmt::IntoSelect<'a>>::Model>
    where
        Q: stmt::IntoSelect<'a>,
    {
        let mut res = self.all(query).await?;

        match res.next().await {
            Some(Ok(value)) => Ok(value),
            Some(Err(err)) => Err(err),
            None => anyhow::bail!("failed to find record"),
        }
    }

    pub async fn delete<'a, Q>(&self, query: Q) -> Result<()>
    where
        Q: stmt::IntoSelect<'a>,
    {
        self.exec(query.into_select().delete()).await?;
        Ok(())
    }

    /// Execute a statement
    pub async fn exec<'a, M: Model>(
        &'a self,
        statement: Statement<'a, M>,
    ) -> Result<ValueStream<'a>> {
        debug!("EXEC: {:#?}", statement);
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
    pub async fn exec_insert_one<'a, M: Model>(&'a self, stmt: stmt::Insert<'a, M>) -> Result<M> {
        // TODO: get rid of this assertion and move to verify
        let toasty_core::stmt::Expr::Record(expr_record) = &stmt.untyped.values else {
            todo!()
        };
        assert!(!expr_record.is_empty());

        // Execute the statement and return the result
        let records = self.exec(stmt.into()).await?;
        let mut cursor = Cursor::new(&self.schema, records);

        cursor.next().await.unwrap()
    }

    /// TODO: remove
    pub async fn reset_db(&self) -> Result<()> {
        self.driver.reset_db(&self.schema).await
    }
}

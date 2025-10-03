use crate::{driver::Driver, engine, Db, DbInner, Model, Result};

use toasty_core::{
    schema::{self, app},
    stmt::{Value, ValueStream},
};
use tokio::sync::oneshot;

use std::sync::Arc;

#[derive(Default)]
pub struct Builder {
    /// Model definitions from macro
    ///
    /// TODO: move this into `core::schema::Builder` after old schema file
    /// implementatin is removed.
    models: Vec<app::Model>,

    /// Schema builder
    core: schema::Builder,
}

impl Builder {
    pub fn register<T: Model>(&mut self) -> &mut Self {
        self.models.push(T::schema());
        self
    }

    /// Set the table name prefix for all tables
    pub fn table_name_prefix(&mut self, prefix: &str) -> &mut Self {
        self.core.table_name_prefix(prefix);
        self
    }

    pub fn build_app_schema(&self) -> Result<app::Schema> {
        app::Schema::from_macro(&self.models)
    }

    pub async fn connect(&mut self, url: &str) -> Result<Db> {
        use crate::driver::Connection;
        self.build(Connection::connect(url).await?).await
    }

    pub async fn build(&mut self, mut driver: impl Driver) -> Result<Db> {
        let schema = self
            .core
            .build(self.build_app_schema()?, driver.capability())?;

        driver.register_schema(&schema.db).await.unwrap();

        let inner = DbInner {
            driver: Arc::new(driver),
            schema: Arc::new(schema),
        };
        let inner2 = inner.clone();

        let (in_tx, mut in_rx) = tokio::sync::mpsc::unbounded_channel::<(
            toasty_core::stmt::Statement,
            oneshot::Sender<Result<ValueStream>>,
        )>();

        let join_handle = tokio::spawn(async move {
            loop {
                let (stmt, tx) = in_rx.recv().await.unwrap();

                match engine::exec(&inner2, stmt).await {
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

        Ok(Db {
            inner,
            in_tx,
            join_handle,
        })
    }
}

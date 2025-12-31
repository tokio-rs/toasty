use crate::{
    db::{Connect, Pool},
    engine::Engine,
    Db, Model, Result,
};

use toasty_core::{
    driver::Driver,
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
        self.build(Connect::new(url)?).await
    }

    pub async fn build(&mut self, driver: impl Driver) -> Result<Db> {
        let pool = Pool::new(driver).await?;

        // Validate capability consistency
        pool.capability().validate()?;

        let schema = self
            .core
            .build(self.build_app_schema()?, pool.capability())?;

        let engine = Engine::new(Arc::new(schema), Arc::new(pool));
        let engine2 = engine.clone();

        let (in_tx, mut in_rx) = tokio::sync::mpsc::unbounded_channel::<(
            toasty_core::stmt::Statement,
            oneshot::Sender<Result<ValueStream>>,
        )>();

        let join_handle = tokio::spawn(async move {
            loop {
                let (stmt, tx) = in_rx.recv().await.unwrap();

                match engine2.exec(stmt).await {
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
            engine,
            in_tx,
            join_handle,
        })
    }
}

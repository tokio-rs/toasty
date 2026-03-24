use crate::{
    db::{Connect, Pool, Shared},
    engine::Engine,
    schema::Register,
    Db, Result,
};

use toasty_core::{
    driver::Driver,
    schema::{self, app},
};

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
    pub fn register<T: Register>(&mut self) -> &mut Self {
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
        let con = Connect::new(url).await?;
        self.build(con).await
    }

    pub async fn build(&mut self, driver: impl Driver) -> Result<Db> {
        let capability = driver.capability();
        capability.validate()?;
        let schema = self.core.build(self.build_app_schema()?, capability)?;

        let engine = Engine::new(Arc::new(schema), capability);
        let pool = Pool::new(driver, engine.clone())?;

        // see if we're able to acquire a valid connection
        let conn = pool.get().await?;
        std::mem::drop(conn);

        Ok(Db {
            shared: Arc::new(Shared { engine, pool }),
            connection: None,
        })
    }
}
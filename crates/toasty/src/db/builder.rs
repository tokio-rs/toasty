use crate::{
    db::{Connect, Pool, Shared},
    engine::Engine,
    Db, Register, Result,
};

use toasty_core::schema::{self, app};

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
        self.build(Connect::new(url)?).await
    }

    pub async fn build(&mut self, driver: impl toasty_core::driver::Driver) -> Result<Db> {
        let pool = Pool::new(driver)?;

        // Validate capability consistency
        pool.capability().validate()?;

        let capability = pool.capability();

        let schema = self
            .core
            .build(self.build_app_schema()?, capability)?;

        let engine = Engine::new(Arc::new(schema), capability);

        Ok(Db {
            shared: Arc::new(Shared { engine, pool }),
            conn: None,
        })
    }
}

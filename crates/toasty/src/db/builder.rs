use crate::{
    db::{Connect, ConnectionSource, Pool},
    Db, Register, Result,
};

use std::sync::Arc;
use toasty_core::{
    driver::Driver,
    schema::{self, app},
};

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

    pub async fn build(&mut self, driver: impl Driver) -> Result<Db> {
        let pool = Pool::new(driver)?;

        // Validate capability consistency
        pool.capability().validate()?;

        let schema = self
            .core
            .build(self.build_app_schema()?, pool.capability())
            .map(Arc::new)?;

        Ok(Db::new(pool.clone(), schema, ConnectionSource::Pool(pool)))
    }
}

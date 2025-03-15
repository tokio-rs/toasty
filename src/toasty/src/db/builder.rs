use super::Db;
use crate::{driver::Driver, Model, Result};

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

        Ok(Db {
            driver: Arc::new(driver),
            schema: Arc::new(schema),
        })
    }
}

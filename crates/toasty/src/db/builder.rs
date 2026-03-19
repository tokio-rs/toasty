use crate::{
    db::{Connect, Pool, Shared},
    engine::Engine,
    Db, Register, Result,
};

use toasty_core::{
    driver::Driver,
    schema::{self, app, Schema},
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

        // Log the schema mapping for debugging
        log_schema(&schema);

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

fn log_schema(schema: &Schema) {
    tracing::info!("=== Schema Mapping ===");

    for model in schema.app.models() {
        let model_id = model.id();
        let model_name = model.name().upper_camel_case();

        match model {
            app::Model::Root(root) => {
                let table = schema.table_for(model_id);
                let table_id = schema.table_id_for(model_id);

                tracing::info!(
                    "Model: {} (ModelId({:?})) → Table: {} (TableId({:?}))",
                    model_name,
                    model_id,
                    table.name,
                    table_id,
                );

                let mapping = schema.mapping_for(model_id);

                for (field_idx, field) in root.fields.iter().enumerate() {
                    let field_name = &field.name.app_name;
                    let field_id = field.id;

                    tracing::info!(
                        "  Field: {}.{} (FieldId({:?}/{:?})) → Type: {:?}",
                        model_name,
                        field_name,
                        field_id.model,
                        field_id.index,
                        field.ty,
                    );

                    // Log column mappings
                    let field_mapping = &mapping.fields[field_idx];
                    for (column_id, _lowering_idx) in field_mapping.columns() {
                        let column = &table.columns[column_id.index];
                        tracing::info!(
                            "    → Column: {} (ColumnId({:?}/{:?})) Type: {:?}",
                            column.name,
                            column_id.table,
                            column_id.index,
                            column.ty,
                        );
                    }
                }
            }
            app::Model::EmbeddedStruct(_) => {
                tracing::info!("Embedded Struct: {} (ModelId({:?}))", model_name, model_id);
            }
            app::Model::EmbeddedEnum(_) => {
                tracing::info!("Embedded Enum: {} (ModelId({:?}))", model_name, model_id);
            }
        }
    }

    tracing::info!("======================");
}

use super::Db;
use crate::{driver::Driver, Model, Result};

use toasty_core::schema::{self, app};

use std::sync::Arc;

#[derive(Default)]
pub struct Builder {
    /// Model definitions from macro (unresolved)
    ///
    /// TODO: move this into `core::schema::Builder` after old schema file
    /// implementatin is removed.
    models: Vec<crate::schema::Model>,

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
        // Convert schema::Model -> app::Model
        let mut app_models = self
            .models
            .iter()
            .enumerate()
            .map(|(index, schema_model)| {
                self.convert_schema_to_app(schema_model, app::ModelId(index))
            })
            .collect::<Result<Vec<_>>>()?;

        // Resolve foreign key target field indices
        self.resolve_foreign_key_targets(&mut app_models)?;

        app::Schema::from_macro(&app_models)
    }

    /// Resolve foreign key target field indices using target field names
    ///
    /// During the initial conversion, we set target field indices to placeholder values
    /// because we need access to all models to resolve field names to indices.
    /// This second pass resolves the actual target field indices.
    fn resolve_foreign_key_targets(&self, app_models: &mut [app::Model]) -> Result<()> {
        // Iterate through all models and their fields to find BelongsTo relations
        for (model_index, schema_model) in self.models.iter().enumerate() {
            for (field_index, schema_field) in schema_model.fields.iter().enumerate() {
                if let crate::schema::FieldTy::BelongsTo(belongs_to) = &schema_field.ty {
                    // Process each foreign key field in this BelongsTo relation
                    for (fk_index, fk_field) in belongs_to.foreign_key.iter().enumerate() {
                        // Look up the target field index by name in the target model
                        // We can safely access by index since models are stored sequentially
                        let target_field_index = app_models[belongs_to.target.0]
                            .fields
                            .iter()
                            .position(|f| f.name == fk_field.target)
                            .unwrap_or_else(|| {
                                panic!(
                                    "Foreign key target field '{}' not found in target model",
                                    fk_field.target
                                )
                            });

                        // Apply the resolution directly by accessing the source model by index
                        // This avoids borrowing conflicts since we're using index-based access
                        if let app::FieldTy::BelongsTo(app_belongs_to) =
                            &mut app_models[model_index].fields[field_index].ty
                        {
                            app_belongs_to.foreign_key.fields[fk_index].target.index =
                                target_field_index;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Convert a schema::Model to app::Model with assigned ModelId
    fn convert_schema_to_app(
        &self,
        schema_model: &crate::schema::Model,
        model_id: app::ModelId,
    ) -> Result<app::Model> {
        // Convert fields
        let fields = schema_model
            .fields
            .iter()
            .enumerate()
            .map(|(field_index, schema_field)| {
                let field_id = app::FieldId {
                    model: model_id,
                    index: field_index,
                };

                let field_ty = match &schema_field.ty {
                    crate::schema::FieldTy::Primitive(primitive) => {
                        app::FieldTy::Primitive(primitive.clone())
                    }
                    crate::schema::FieldTy::BelongsTo(belongs_to) => {
                        // Convert field names to FieldId references
                        let foreign_key_fields = belongs_to
                            .foreign_key
                            .iter()
                            .map(|fk_field| {
                                // Find the source field index by name within this model
                                let source_field_index = schema_model
                                    .fields
                                    .iter()
                                    .position(|f| f.name == fk_field.source)
                                    .unwrap_or_else(|| {
                                        panic!(
                                            "Foreign key source field '{}' not found in model",
                                            fk_field.source
                                        )
                                    });

                                // Store target field name for later resolution
                                // We use usize::MAX as a placeholder since we need access to all models
                                // to resolve target field names to indices (done in resolve_foreign_key_targets)
                                app::ForeignKeyField {
                                    source: app::FieldId {
                                        model: model_id,
                                        index: source_field_index,
                                    },
                                    target: app::FieldId {
                                        model: belongs_to.target,
                                        index: usize::MAX, // Placeholder - will be resolved later
                                    },
                                }
                            })
                            .collect();

                        let foreign_key = app::ForeignKey {
                            fields: foreign_key_fields,
                        };

                        app::FieldTy::BelongsTo(app::BelongsTo {
                            target: belongs_to.target,
                            expr_ty: belongs_to.expr_ty.clone(),
                            pair: None, // Resolved later
                            foreign_key,
                        })
                    }
                    crate::schema::FieldTy::HasMany(has_many) => {
                        app::FieldTy::HasMany(app::HasMany {
                            target: has_many.target,
                            expr_ty: has_many.expr_ty.clone(),
                            singular: has_many.singular.clone(),
                            pair: app::FieldId {
                                model: app::ModelId(usize::MAX),
                                index: usize::MAX,
                            }, // Placeholder
                        })
                    }
                    crate::schema::FieldTy::HasOne(has_one) => {
                        app::FieldTy::HasOne(app::HasOne {
                            target: has_one.target,
                            expr_ty: has_one.expr_ty.clone(),
                            pair: app::FieldId {
                                model: app::ModelId(usize::MAX),
                                index: usize::MAX,
                            }, // Placeholder
                        })
                    }
                };

                app::Field {
                    id: field_id,
                    name: schema_field.name.clone(),
                    ty: field_ty,
                    nullable: schema_field.nullable,
                    primary_key: schema_field.primary_key,
                    auto: schema_field.auto.clone(),
                    constraints: schema_field.constraints.clone(),
                }
            })
            .collect();

        // Convert primary key
        let primary_key = app::PrimaryKey {
            fields: schema_model
                .primary_key
                .fields
                .iter()
                .map(|&field_index| app::FieldId {
                    model: model_id,
                    index: field_index,
                })
                .collect(),
            index: app::IndexId {
                model: model_id,
                index: 0,
            },
        };

        // Convert indices
        let indices = schema_model
            .indices
            .iter()
            .enumerate()
            .map(|(index_idx, schema_index)| {
                let index_fields = schema_index
                    .fields
                    .iter()
                    .map(|schema_index_field| {
                        app::IndexField {
                            field: app::FieldId {
                                model: model_id,
                                index: schema_index_field.field,
                            },
                            op: crate::schema::db::IndexOp::Eq, // Default operation
                            scope: schema_index_field.scope,
                        }
                    })
                    .collect();

                app::Index {
                    id: app::IndexId {
                        model: model_id,
                        index: index_idx,
                    },
                    fields: index_fields,
                    unique: schema_index.unique,
                    primary_key: schema_index.primary_key,
                }
            })
            .collect();

        Ok(app::Model {
            id: model_id,
            name: schema_model.name.clone(),
            fields,
            primary_key,
            indices,
            table_name: schema_model.table_name.clone(),
        })
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

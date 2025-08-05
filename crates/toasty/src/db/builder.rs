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
        // Build TypeId to ModelId mapping with sequential assignment
        let type_to_model_mapping = self.build_sequential_type_to_model_mapping();

        // Convert schema::Model -> app::Model using sequential ModelIds
        let app_models = self
            .models
            .iter()
            .enumerate()
            .map(|(index, schema_model)| {
                let model_id = app::ModelId(index); // Sequential assignment!
                self.convert_schema_to_app(schema_model, model_id, &type_to_model_mapping)
            })
            .collect::<Result<Vec<_>>>()?;

        app::Schema::from_macro_with_mapping(&app_models, type_to_model_mapping)
    }

    /// Build a mapping from TypeId to sequential ModelId based on registration order
    fn build_sequential_type_to_model_mapping(
        &self,
    ) -> std::collections::HashMap<std::any::TypeId, app::ModelId> {
        self.models
            .iter()
            .enumerate()
            .map(|(index, schema_model)| (schema_model.type_id, app::ModelId(index)))
            .collect()
    }

    /// Convert a schema::Model to app::Model with assigned ModelId
    fn convert_schema_to_app(
        &self,
        schema_model: &crate::schema::Model,
        model_id: app::ModelId,
        type_to_model_mapping: &std::collections::HashMap<std::any::TypeId, app::ModelId>,
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
                        // Convert TypeId to ModelId using the mapping
                        let target_model_id = type_to_model_mapping
                            .get(&belongs_to.target)
                            .copied()
                            .unwrap_or_else(|| {
                                panic!("TypeId {:?} not found in mapping", belongs_to.target)
                            });

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
                                        model: target_model_id,
                                        index: usize::MAX, // Placeholder - will be resolved later
                                    },
                                }
                            })
                            .collect();

                        let foreign_key = app::ForeignKey {
                            fields: foreign_key_fields,
                        };

                        app::FieldTy::BelongsTo(app::BelongsTo {
                            target: target_model_id,
                            expr_ty: belongs_to.expr_ty.clone(),
                            pair: None, // Resolved later
                            foreign_key,
                        })
                    }
                    crate::schema::FieldTy::HasMany(has_many) => {
                        // Convert TypeId to ModelId using the mapping
                        let target_model_id = type_to_model_mapping
                            .get(&has_many.target)
                            .copied()
                            .unwrap_or_else(|| {
                                panic!("TypeId {:?} not found in mapping", has_many.target)
                            });

                        app::FieldTy::HasMany(app::HasMany {
                            target: target_model_id,
                            expr_ty: has_many.expr_ty.clone(),
                            singular: has_many.singular.clone(),
                            pair: app::FieldId {
                                model: app::ModelId(usize::MAX),
                                index: usize::MAX,
                            }, // Placeholder
                        })
                    }
                    crate::schema::FieldTy::HasOne(has_one) => {
                        // Convert TypeId to ModelId using the mapping
                        let target_model_id = type_to_model_mapping
                            .get(&has_one.target)
                            .copied()
                            .unwrap_or_else(|| {
                                panic!("TypeId {:?} not found in mapping", has_one.target)
                            });

                        app::FieldTy::HasOne(app::HasOne {
                            target: target_model_id,
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

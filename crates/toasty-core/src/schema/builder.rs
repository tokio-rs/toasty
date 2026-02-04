mod table;

use super::{app, db, mapping, Result};
use crate::schema::mapping::TableToModel;
use crate::schema::{ColumnId, Mapping, Schema, Table, TableId};
use crate::{driver, stmt};
use indexmap::IndexMap;
use std::sync::Arc;

#[derive(Debug)]
pub struct Builder {
    /// If set, prefix all table names with this string
    table_name_prefix: Option<String>,
}

/// Used to track state during the build process
struct BuildSchema<'a> {
    /// Build options
    builder: &'a Builder,

    db: &'a driver::Capability,

    /// Maps table names to identifiers. The identifiers are reserved before the
    /// table objects are actually created.
    table_lookup: IndexMap<String, TableId>,

    /// Tables as they are built
    tables: Vec<Table>,

    /// App-level to db-level schema mapping
    mapping: Mapping,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            table_name_prefix: None,
        }
    }

    pub fn table_name_prefix(&mut self, prefix: &str) -> &mut Self {
        self.table_name_prefix = Some(prefix.to_string());
        self
    }

    pub fn build(&self, mut app: app::Schema, db: &driver::Capability) -> Result<Schema> {
        let mut builder = BuildSchema {
            builder: self,
            db,
            table_lookup: IndexMap::new(),
            tables: vec![],
            mapping: Mapping {
                models: IndexMap::new(),
            },
        };

        for model in app.models.values_mut() {
            // Initial verification pass to ensure all models are valid based on the
            // specified driver capability.
            model.verify(db)?;

            // Generate any additional field-level constraints to satisfy the
            // target database.
            builder.build_model_constraints(model)?;
        }

        // Find all models that specified a table name, ensure a table is
        // created for that model, and link the model with the table.
        // Skip embedded models as they don't have their own tables.
        for model in app.models() {
            // Skip embedded models - they are flattened into parent tables
            if model.is_embedded() {
                continue;
            }

            let table = builder.build_table_stub_for_model(model);

            // Create a mapping stub for the model
            builder.mapping.models.insert(
                model.id,
                mapping::Model {
                    id: model.id,
                    table,
                    columns: vec![],
                    // Create a mapping stub for each field
                    fields: model
                        .fields
                        .iter()
                        .map(|field| match &field.ty {
                            app::FieldTy::Primitive(_) => {
                                mapping::Field::Primitive(mapping::FieldPrimitive {
                                    column: ColumnId::placeholder(),
                                    lowering: 0,
                                })
                            }
                            app::FieldTy::Embedded(_) => {
                                mapping::Field::Embedded(mapping::FieldEmbedded { fields: vec![] })
                            }
                            _ => mapping::Field::Relation,
                        })
                        .collect(),
                    model_to_table: stmt::ExprRecord::default(),
                    model_pk_to_table: stmt::Expr::null(),
                    table_to_model: TableToModel::default(),
                },
            );
        }

        builder.build_tables_from_models(&app, db);

        let schema = Schema {
            app,
            db: Arc::new(db::Schema {
                tables: builder.tables,
            }),
            mapping: builder.mapping,
        };

        // Verify the schema structure
        schema.verify()?;

        Ok(schema)
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildSchema<'_> {
    fn build_model_constraints(&self, model: &mut app::Model) -> Result<()> {
        for field in model.fields.iter_mut() {
            if let app::FieldTy::Primitive(primitive) = &mut field.ty {
                let storage_ty = db::Type::from_app(
                    &primitive.ty,
                    primitive.storage_ty.as_ref(),
                    &self.db.storage_types,
                )?;

                if let db::Type::VarChar(size) = storage_ty {
                    field
                        .constraints
                        .push(app::Constraint::length_less_than(size));
                }
            }
        }

        Ok(())
    }
}

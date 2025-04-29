mod table;

use super::*;

use indexmap::IndexMap;

#[derive(Debug)]
pub struct Builder {
    /// If set, prefix all table names with this string
    table_name_prefix: Option<String>,
}

/// Used to track state during the build process
struct BuildSchema<'a> {
    /// Build options
    builder: &'a Builder,

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

    pub fn build(&self, app: app::Schema, db: &driver::Capability) -> Result<Schema> {
        let mut builder = BuildSchema {
            builder: self,
            table_lookup: IndexMap::new(),
            tables: vec![],
            mapping: Mapping {
                models: IndexMap::new(),
            },
        };

        // Initial verification pass to ensure all models are valid based on the
        // specified driver capability.
        for model in app.models() {
            model.verify(db)?;
        }

        // Find all models that specified a table name, ensure a table is
        // created for that model, and link the model with the table.
        for model in app.models() {
            let table = builder.build_table_stub_for_model(model);

            // Create a mapping stub for the model
            builder.mapping.models.insert(
                model.id,
                mapping::Model {
                    id: model.id,
                    table,
                    columns: vec![],
                    // Create a mapping stub for each primitive field
                    fields: model
                        .fields
                        .iter()
                        .map(|field| match &field.ty {
                            app::FieldTy::Primitive(_) => Some(mapping::Field {
                                column: ColumnId::placeholder(),
                                lowering: 0,
                            }),
                            _ => None,
                        })
                        .collect(),
                    model_to_table: stmt::ExprRecord::default(),
                    model_pk_to_table: stmt::Expr::default(),
                    table_to_model: stmt::ExprRecord::default(),
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
        schema.verify();

        Ok(schema)
    }
}

impl Default for Builder {
    fn default() -> Builder {
        Builder::new()
    }
}

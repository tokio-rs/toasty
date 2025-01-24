mod table;

use super::*;

use std::collections::HashMap;

/// Used to resolve types during parsing
pub(crate) struct Builder {
    /// Maps table names to identifiers. The identifiers are reserved before the
    /// table objects are actually created.
    table_lookup: HashMap<String, TableId>,

    // ----- OLD -----
    /// Tables as they are built
    pub(crate) tables: Vec<Table>,

    /// App-level to db-level schema mapping
    pub(crate) mapping: Mapping,
}

impl Builder {
    pub(crate) fn from_ast(ast: &ast::Schema) -> crate::Result<Schema> {
        let app = app::Schema::from_ast(ast)?;
        let mut builder = Builder {
            table_lookup: HashMap::new(),

            tables: vec![],
            mapping: Mapping { models: vec![] },
        };

        // Find all models that specified a table name, ensure a table is
        // created for that model, and link the model with the table.
        for model in &app.models {
            let table = if let Some(table_name) = &model.table_name {
                if !builder.table_lookup.contains_key(table_name) {
                    let id = builder.register_table(&table_name);
                    builder.tables.push(Table::new(id, table_name.clone()));
                }

                builder.table_lookup.get(table_name).unwrap().clone()
            } else {
                builder.build_table_stub_for_model(model)
            };

            // Create a mapping stub for the model
            builder.mapping.models.push(mapping::Model {
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
            });
        }

        builder.build_tables_from_models(&app);

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

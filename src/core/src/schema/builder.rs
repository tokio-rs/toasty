use super::*;

/// Used to resolve types during parsing
#[derive(Default)]
pub(crate) struct Builder {
    /// Tables as they are built
    pub(crate) tables: Vec<Table>,
}

impl Builder {
    pub(crate) fn from_ast(mut self, ast: &ast::Schema) -> crate::Result<Schema> {
        let mut ctx = db::Context::new();

        let mut app_schema = app::Schema::from_ast(ast)?;

        // Find all models that specified a table name, ensure a table is
        // created for that model, and link the model with the table.
        for model in &mut app_schema.models {
            let Some(table_name) = &model.table_name else {
                continue;
            };

            let contains_table = self
                .tables
                .iter()
                .find(|table| table.name == *table_name)
                .is_some();

            if !contains_table {
                let id = ctx.register_table(&table_name);
                self.tables.push(Table::new(id, table_name.clone()));
            }

            let table = self
                .tables
                .iter()
                .find(|table| table.name == *table_name)
                .unwrap();

            model.lowering.table = table.id;
        }

        // Find all defined tables and generate their schema based on assigned
        // models
        for table in &mut self.tables {
            let mut models = app_schema
                .models
                .iter_mut()
                .filter(|model| model.lowering.table == table.id)
                .collect::<Vec<_>>();
            table.lower_models(&mut models);
        }

        // Now, we can initialize tables for each model. Mutability is needed so
        // that the model can be updated to reference the newly created tables
        // and columns.
        for model in &mut app_schema.models {
            if model.lowering.table != TableId::placeholder() {
                continue;
            }

            let table = Table::from_model(&mut ctx, model)?;
            assert_eq!(self.tables.len(), table.id.0);
            self.tables.push(table);
        }

        let schema = Schema {
            app: app_schema,
            db: Arc::new(db::Schema {
                tables: self.tables,
            }),
        };

        // Verify the schema structure
        schema.verify();

        Ok(schema)
    }
}

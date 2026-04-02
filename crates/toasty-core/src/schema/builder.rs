mod table;

use super::{Result, app, db, mapping};
use crate::schema::mapping::TableToModel;
use crate::schema::{Mapping, Schema, Table, TableId};
use crate::{driver, stmt};
use indexmap::IndexMap;

/// Constructs a [`Schema`] from an app-level schema and driver capabilities.
///
/// The builder generates the database-level schema (tables, columns, indices)
/// and the mapping layer that connects app fields to database columns. Call
/// [`build`](Builder::build) to produce the final, validated [`Schema`].
///
/// # Examples
///
/// ```ignore
/// use toasty_core::schema::Builder;
///
/// let schema = Builder::new()
///     .table_name_prefix("myapp_")
///     .build(app_schema, &capability)
///     .expect("valid schema");
/// ```
#[derive(Debug)]
pub struct Builder {
    /// If set, prefix all table names with this string.
    table_name_prefix: Option<String>,
}

/// Used to track state during the build process.
struct BuildSchema<'a> {
    /// Build options.
    builder: &'a Builder,

    db: &'a driver::Capability,

    /// Maps table names to identifiers. The identifiers are reserved before the
    /// table objects are actually created.
    table_lookup: IndexMap<String, TableId>,

    /// Tables as they are built.
    tables: Vec<Table>,

    /// App-level to db-level schema mapping.
    mapping: Mapping,
}

impl Builder {
    /// Creates a new `Builder` with default settings.
    ///
    /// # Examples
    ///
    /// ```
    /// use toasty_core::schema::Builder;
    ///
    /// let builder = Builder::new();
    /// ```
    pub fn new() -> Self {
        Self {
            table_name_prefix: None,
        }
    }

    /// Sets a prefix that will be prepended to all generated table names.
    ///
    /// This is useful for multi-tenant setups or avoiding name collisions.
    ///
    /// # Examples
    ///
    /// ```
    /// use toasty_core::schema::Builder;
    ///
    /// let mut builder = Builder::new();
    /// builder.table_name_prefix("myapp_");
    /// ```
    pub fn table_name_prefix(&mut self, prefix: &str) -> &mut Self {
        self.table_name_prefix = Some(prefix.to_string());
        self
    }

    /// Builds the complete [`Schema`] from the given app schema and driver
    /// capabilities.
    ///
    /// This method:
    /// 1. Verifies each model against the driver's capabilities
    /// 2. Generates field-level constraints (e.g., `VARCHAR` length limits)
    /// 3. Creates database tables, columns, and indices
    /// 4. Builds the bidirectional mapping between models and tables
    /// 5. Validates the resulting schema
    ///
    /// # Errors
    ///
    /// Returns an error if the schema is invalid (e.g., duplicate index names,
    /// unsupported types, missing references).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use toasty_core::schema::Builder;
    ///
    /// let schema = Builder::new()
    ///     .build(app_schema, &capability)?;
    /// ```
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
            let app::Model::Root(model) = model else {
                continue;
            };

            let table = builder.build_table_stub_for_model(model);

            // Create a mapping shell for the model (fields will be built during mapping phase)
            builder.mapping.models.insert(
                model.id,
                mapping::Model {
                    id: model.id,
                    table,
                    columns: vec![],
                    fields: vec![], // Will be populated during mapping phase
                    model_to_table: stmt::ExprRecord::default(),
                    table_to_model: TableToModel::default(),
                },
            );
        }

        builder.build_tables_from_models(&app, db)?;

        let schema = Schema {
            app,
            db: db::Schema {
                tables: builder.tables,
            },
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
        let fields = match model {
            app::Model::Root(root) => &mut root.fields[..],
            app::Model::EmbeddedStruct(embedded) => &mut embedded.fields[..],
            app::Model::EmbeddedEnum(_) => return Ok(()),
        };
        for field in fields.iter_mut() {
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

mod table;

use super::{Result, app, db, mapping};
use crate::schema::mapping::TableToModel;
use crate::schema::{Mapping, Schema, Table, TableId};
use crate::{driver, stmt};
use indexmap::IndexMap;
use std::collections::HashSet;

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
        // Skip item-collection children — they share the parent's table,
        // which gets resolved in the fixup pass below.
        for model in app.models() {
            let app::Model::Root(model) = model else {
                continue;
            };

            // Item-collection children share the parent's table; assign a
            // placeholder now and fix it up after all root tables exist.
            let table = if model.parent.is_some() {
                TableId::placeholder()
            } else {
                builder.build_table_stub_for_model(model)
            };

            builder.mapping.models.insert(
                model.id,
                mapping::Model {
                    id: model.id,
                    table,
                    columns: vec![],
                    fields: vec![],
                    model_to_table: stmt::ExprRecord::default(),
                    table_to_model: TableToModel::default(),
                    default_returning: stmt::Expr::null(),
                    item_collection: mapping::ItemCollection::default(),
                },
            );
        }

        // Fix up item-collection children: point them at the root model's table
        // and populate the FK→PK field mapping used during column building.
        builder.fixup_item_collection_mappings(&app)?;

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
    /// Resolve table IDs and FK→PK field mappings for item-collection children.
    ///
    /// Walks the ancestry chain of each child until it reaches a root (a model
    /// with no `parent` pointer), then assigns that root's table to
    /// every model in the chain.
    fn fixup_item_collection_mappings(&mut self, app: &app::Schema) -> Result<()> {
        // Collect child model IDs first to avoid borrow issues.
        let children: Vec<_> = app
            .models()
            .filter_map(|m| {
                let r = m.as_root()?;
                r.parent.map(|_| r.id)
            })
            .collect();

        for child_id in children {
            // Walk up to root.
            let table = self.resolve_item_collection_table(app, child_id);
            self.mapping.model_mut(child_id).table = table;

            // Build FK-source → parent-PK field mapping.
            let child_root = app.model(child_id).as_root_unwrap();
            let field_mapping: indexmap::IndexMap<_, _> = child_root
                .fields
                .iter()
                .filter_map(|f| f.ty.as_belongs_to())
                .flat_map(|bt| {
                    bt.foreign_key
                        .fields
                        .iter()
                        .map(|fkf| (fkf.source, fkf.target))
                })
                .filter(|(src, _)| child_root.fields[src.index].primary_key)
                .collect();

            self.mapping
                .model_mut(child_id)
                .item_collection
                .field_mapping = field_mapping;
        }

        Ok(())
    }

    fn resolve_item_collection_table(
        &self,
        app: &app::Schema,
        model_id: app::ModelId,
    ) -> crate::schema::TableId {
        let root = app.model(model_id).as_root_unwrap();
        match root.parent {
            None => self.mapping.model(model_id).table,
            Some(parent_id) => self.resolve_item_collection_table(app, parent_id),
        }
    }

    fn build_model_constraints(&self, model: &mut app::Model) -> Result<()> {
        let model_name = model.name().to_string();

        // Collect Bool fields used as key/index attributes on backends that
        // don't support BOOL as a key attribute type (e.g. DynamoDB). These
        // need db::Type::Integer(1) as their storage type.
        let mut bool_key_fields: HashSet<app::FieldId> = HashSet::new();
        if let app::Model::Root(root) = &*model
            && !self.db.bool_key_type
        {
            let index_key_fields: HashSet<app::FieldId> = root
                .indices
                .iter()
                .flat_map(|idx| idx.fields.iter().map(|f| f.field))
                .collect();
            for f in &root.fields {
                if (f.primary_key || index_key_fields.contains(&f.id))
                    && matches!(&f.ty, app::FieldTy::Primitive(p) if matches!(p.ty, stmt::Type::Bool))
                {
                    bool_key_fields.insert(f.id);
                }
            }
        }

        let fields = match model {
            app::Model::Root(root) => &mut root.fields[..],
            app::Model::EmbeddedStruct(embedded) => &mut embedded.fields[..],
            app::Model::EmbeddedEnum(_) => return Ok(()),
        };
        for field in fields.iter_mut() {
            if let app::FieldTy::Primitive(primitive) = &mut field.ty {
                // On backends that don't support BOOL as a key attribute type,
                // store Bool key/index fields as Integer(1). The engine's
                // cast mechanism converts Bool ↔ I8 transparently; the driver
                // never needs to special-case bools-as-numbers.
                if bool_key_fields.contains(&field.id) {
                    primitive.storage_ty = Some(db::Type::Integer(1));
                }

                if matches!(primitive.ty, stmt::Type::List(_)) && !self.db.vec_scalar {
                    let field_name = field.name.app.as_deref().unwrap_or_else(|| {
                        panic!(
                            "model `{model_name}` field has no app-level name; \
                             expected every primitive field to carry one"
                        )
                    });
                    return Err(crate::Error::unsupported_feature(format!(
                        "model `{model_name}` field `{field_name}` is a `Vec<T>` collection, \
                         but this backend does not yet support `Vec<scalar>` model fields."
                    )));
                }

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

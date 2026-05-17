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

        // Resolve `#[document]` collection fields — `Type::List(Model(id))` as
        // emitted by the `Document` derive trait — into the self-describing
        // `Type::List(Document(..))` shape, now that every embed is registered
        // and its field names are known. Must run before `from_app`, which has
        // no mapping for a bare `Type::Model` element.
        resolve_document_types(&mut app)?;

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
                    default_returning: stmt::Expr::null(),
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
        let model_name = model.name().to_string();
        let fields = match model {
            app::Model::Root(root) => &mut root.fields[..],
            app::Model::EmbeddedStruct(embedded) => &mut embedded.fields[..],
            app::Model::EmbeddedEnum(_) => return Ok(()),
        };
        for field in fields.iter_mut() {
            if let app::FieldTy::Primitive(primitive) = &mut field.ty {
                if let stmt::Type::List(elem) = &primitive.ty {
                    let field_name = || {
                        field.name.app.as_deref().unwrap_or_else(|| {
                            panic!(
                                "model `{model_name}` field has no app-level name; \
                                 expected every primitive field to carry one"
                            )
                        })
                    };

                    if matches!(**elem, stmt::Type::Document(_)) {
                        // A `#[document]` collection of embedded structs.
                        if !self.db.document_collections {
                            return Err(crate::Error::unsupported_feature(format!(
                                "model `{model_name}` field `{}` is a `#[document]` \
                                 collection, but this backend does not yet support \
                                 `#[document]` collection fields.",
                                field_name()
                            )));
                        }
                    } else if !self.db.vec_scalar {
                        return Err(crate::Error::unsupported_feature(format!(
                            "model `{model_name}` field `{}` is a `Vec<T>` collection, \
                             but this backend does not yet support `Vec<scalar>` model fields.",
                            field_name()
                        )));
                    }
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

/// Rewrite `#[document]` collection fields from the macro-emitted
/// `Type::List(Model(id))` shape into the self-describing
/// `Type::List(Document(..))` shape.
///
/// The macro cannot name the embed's fields — it only knows the element type
/// — so it emits `Type::Model(id)` for the element. By schema-build time every
/// embed is registered, so the builder can walk the embed's fields and produce
/// a [`stmt::TypeDocument`] carrying the field names the JSON codec needs.
fn resolve_document_types(app: &mut app::Schema) -> Result<()> {
    // Read-only pass: collect `(model_id, field_index, resolved_ty)` patches.
    let mut patches = Vec::new();

    for model in app.models.values() {
        let fields = match model {
            app::Model::Root(root) => &root.fields,
            app::Model::EmbeddedStruct(embedded) => &embedded.fields,
            app::Model::EmbeddedEnum(_) => continue,
        };

        for (index, field) in fields.iter().enumerate() {
            let app::FieldTy::Primitive(primitive) = &field.ty else {
                continue;
            };
            let stmt::Type::List(elem) = &primitive.ty else {
                continue;
            };
            let stmt::Type::Model(embed_id) = &**elem else {
                continue;
            };

            let document = resolve_document_ty(&app.models, *embed_id)?;
            patches.push((
                model.id(),
                index,
                stmt::Type::List(Box::new(stmt::Type::Document(document))),
            ));
        }
    }

    // Mutable pass: apply the patches.
    for (model_id, index, ty) in patches {
        let model = app.models.get_mut(&model_id).expect("model id from map");
        let fields = match model {
            app::Model::Root(root) => &mut root.fields,
            app::Model::EmbeddedStruct(embedded) => &mut embedded.fields,
            app::Model::EmbeddedEnum(_) => unreachable!(),
        };
        if let app::FieldTy::Primitive(primitive) = &mut fields[index].ty {
            primitive.ty = ty;
        }
    }

    Ok(())
}

/// Build a [`stmt::TypeDocument`] for the embedded struct `model_id` by walking
/// its fields. Recurses through nested embedded structs.
fn resolve_document_ty(
    models: &IndexMap<app::ModelId, app::Model>,
    model_id: app::ModelId,
) -> Result<stmt::TypeDocument> {
    let app::Model::EmbeddedStruct(embedded) = &models[&model_id] else {
        return Err(crate::Error::unsupported_feature(
            "#[document] collection elements must be `#[derive(Embed)]` structs",
        ));
    };

    let mut doc_fields = Vec::with_capacity(embedded.fields.len());

    for field in &embedded.fields {
        let name = field.name.app.clone().ok_or_else(|| {
            crate::Error::unsupported_feature(format!(
                "embedded struct `{}` has an unnamed field; #[document] storage \
                 requires named fields",
                embedded.name
            ))
        })?;

        // A `#[column(\"...\")]` rename has no meaning under document storage —
        // document keys come from the Rust field name.
        if field.name.storage.is_some() {
            return Err(crate::Error::unsupported_feature(format!(
                "embedded struct `{}` field `{name}` has a `#[column]` rename, \
                 which is not supported inside a #[document] field",
                embedded.name
            )));
        }

        let ty = match &field.ty {
            app::FieldTy::Primitive(primitive) => match &primitive.ty {
                // A `#[document]` collection nested inside the embed.
                stmt::Type::List(elem) if matches!(**elem, stmt::Type::Model(_)) => {
                    let stmt::Type::Model(nested) = &**elem else {
                        unreachable!()
                    };
                    stmt::Type::List(Box::new(stmt::Type::Document(resolve_document_ty(
                        models, *nested,
                    )?)))
                }
                other => other.clone(),
            },
            // A nested column-expanded embed becomes a nested document.
            app::FieldTy::Embedded(embedded_field) => {
                stmt::Type::Document(resolve_document_ty(models, embedded_field.target)?)
            }
            app::FieldTy::BelongsTo(_) | app::FieldTy::HasMany(_) | app::FieldTy::HasOne(_) => {
                return Err(crate::Error::unsupported_feature(format!(
                    "embedded struct `{}` field `{name}` is a relation, which is \
                     not supported inside a #[document] field",
                    embedded.name
                )));
            }
        };

        doc_fields.push(stmt::DocumentField { name, ty });
    }

    Ok(stmt::TypeDocument::new(doc_fields))
}

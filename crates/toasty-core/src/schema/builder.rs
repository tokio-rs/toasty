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
                document_columns: IndexMap::new(),
            },
        };

        // Validate `#[document]` embeds now that every embed is registered.
        // A document column is typed by the structural `Type::Object`; its
        // embedded model (`Type::Model`) is recorded in the mapping's
        // document-column index and resolved on demand, so there is nothing
        // to rewrite — only to check (named fields, no `#[column]` rename, no
        // relations, no unrepresentable leaf).
        verify_document_types(&app)?;

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
        builder.index_document_columns(&app);

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
    /// Populates [`Mapping::document_columns`]: for every `#[document]` field
    /// — including fields nested inside column-expanded embedded structs and
    /// embedded enum variants — record the field's app-level type
    /// (`Type::Model` or `List(Model)`) against the column that stores it.
    ///
    /// The column itself is typed by the structural `stmt::Type::Object`
    /// (columns don't know about models); this index is where the engine
    /// recovers the embedded-model view of a document column.
    fn index_document_columns(&mut self, app: &app::Schema) {
        fn collect_field(
            app: &app::Schema,
            field: &app::Field,
            mapped: &mapping::Field,
            out: &mut IndexMap<db::ColumnId, stmt::Type>,
        ) {
            match (&field.ty, mapped) {
                (app::FieldTy::Primitive(primitive), mapping::Field::Primitive(p))
                    if document_embed_id(&primitive.ty).is_some() =>
                {
                    out.insert(p.column, primitive.ty.clone());
                }
                (app::FieldTy::Embedded(_), mapping::Field::Struct(s)) => {
                    for (field, mapped) in app.fields(s.id).iter().zip(&s.fields) {
                        collect_field(app, field, mapped, out);
                    }
                }
                (app::FieldTy::Embedded(embedded), mapping::Field::Enum(e)) => {
                    let app::Model::EmbeddedEnum(embedded_enum) = app.model(embedded.target) else {
                        panic!("enum field mapping on a non-enum embed")
                    };
                    for (index, variant) in e.variants.iter().enumerate() {
                        for (field, mapped) in
                            embedded_enum.variant_fields(index).zip(&variant.fields)
                        {
                            collect_field(app, field, mapped, out);
                        }
                    }
                }
                _ => {}
            }
        }

        let mut out = IndexMap::new();
        for model_mapping in self.mapping.models.values() {
            for (field, mapped) in app
                .fields(model_mapping.id)
                .iter()
                .zip(&model_mapping.fields)
            {
                collect_field(app, field, mapped, &mut out);
            }
        }
        self.mapping.document_columns = out;
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

                // `#[document]` storage covers a bare embedded struct
                // (`Type::Model`) and a collection of embedded structs
                // (`Type::List(Model)`); both are gated by the same capability.
                // A plain `Vec<scalar>` has its own gate.
                let field_name = || {
                    field.name.app.as_deref().unwrap_or_else(|| {
                        panic!(
                            "model `{model_name}` field has no app-level name; \
                             expected every primitive field to carry one"
                        )
                    })
                };

                let is_document =
                    document_embed_id(&primitive.ty).is_some() || primitive.ty.is_json();

                if is_document {
                    if !self.db.document_collections {
                        return Err(crate::Error::unsupported_feature(format!(
                            "model `{model_name}` field `{}` uses document storage, \
                             but this backend does not support document fields.",
                            field_name()
                        )));
                    }
                    // The embed's structure and leaf types are validated up front
                    // by `verify_document_types` (it can recurse into nested
                    // embeds via the schema, which this per-field pass cannot).
                } else if matches!(&primitive.ty, stmt::Type::List(_)) && !self.db.vec_scalar {
                    return Err(crate::Error::unsupported_feature(format!(
                        "model `{model_name}` field `{}` is a `Vec<T>` collection, \
                         but this backend does not yet support `Vec<scalar>` model fields.",
                        field_name()
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

/// The name of a `#[document]` leaf scalar type that JSON document storage
/// cannot represent, or `None` if it is supported. Recurses through list
/// element types; embeds are walked separately by [`verify_document_embed`].
/// `Zoned` is rejected because no backend can round-trip its `[IANA]`
/// annotation; `Bytes` because JSON has no binary representation.
fn document_unsupported_leaf(ty: &stmt::Type) -> Option<&'static str> {
    match ty {
        #[cfg(feature = "jiff")]
        stmt::Type::Zoned => Some("Zoned"),
        stmt::Type::Bytes => Some("Vec<u8>"),
        stmt::Type::List(elem) => document_unsupported_leaf(elem),
        _ => None,
    }
}

/// The embedded-struct id a `#[document]` column stores, if any: `Type::Model`
/// for a bare embed, `List(Model)` for a collection. `None` for a scalar or
/// `Vec<scalar>` column.
fn document_embed_id(ty: &stmt::Type) -> Option<app::ModelId> {
    match ty {
        stmt::Type::Model(id) => Some(*id),
        stmt::Type::List(elem) => match &**elem {
            stmt::Type::Model(id) => Some(*id),
            _ => None,
        },
        _ => None,
    }
}

/// Validate every `#[document]` column's embedded struct. A document column
/// tracks its shape as `Type::Model(embed_id)` and resolves the embed's fields
/// on demand from the embedded model, so there is nothing to rewrite — only to
/// check that the embed is JSON-encodable.
fn verify_document_types(app: &app::Schema) -> Result<()> {
    for model in app.models.values() {
        let fields = match model {
            app::Model::Root(root) => &root.fields,
            app::Model::EmbeddedStruct(embedded) => &embedded.fields,
            app::Model::EmbeddedEnum(_) => continue,
        };

        for field in fields {
            if let app::FieldTy::Primitive(primitive) = &field.ty
                && let Some(embed_id) = document_embed_id(&primitive.ty)
            {
                verify_document_embed(app, embed_id)?;
            }
        }
    }

    Ok(())
}

/// Recursively validate an embedded struct used inside a `#[document]`: every
/// field must be named, free of a `#[column]` rename, not a relation, and have
/// a JSON-encodable leaf type. Nested embeds (bare, collection, or
/// column-expanded) are validated as nested documents.
fn verify_document_embed(app: &app::Schema, embed_id: app::ModelId) -> Result<()> {
    let app::Model::EmbeddedStruct(embedded) = app.model(embed_id) else {
        return Err(crate::Error::unsupported_feature(
            "#[document] elements must be `#[derive(Embed)]` structs",
        ));
    };

    for field in &embedded.fields {
        let Some(name) = field.name.app.as_deref() else {
            return Err(crate::Error::unsupported_feature(format!(
                "embedded struct `{}` has an unnamed field; #[document] storage \
                 requires named fields",
                embedded.name
            )));
        };

        // A `#[column("...")]` rename has no meaning under document storage —
        // document keys come from the Rust field name.
        if field.name.storage.is_some() {
            return Err(crate::Error::unsupported_feature(format!(
                "embedded struct `{}` field `{name}` has a `#[column]` rename, \
                 which is not supported inside a #[document] field",
                embedded.name
            )));
        }

        match &field.ty {
            app::FieldTy::Primitive(primitive) => {
                // A nested embed (bare or collection) is itself a nested
                // document; recurse. Otherwise check the scalar leaf.
                if let Some(nested) = document_embed_id(&primitive.ty) {
                    verify_document_embed(app, nested)?;
                } else if let Some(bad) = document_unsupported_leaf(&primitive.ty) {
                    return Err(crate::Error::unsupported_feature(format!(
                        "embedded struct `{}` field `{name}` stores `{bad}` inside a \
                         `#[document]`, which JSON document storage cannot represent.",
                        embedded.name
                    )));
                }
            }
            // A nested column-expanded embed becomes a nested document.
            app::FieldTy::Embedded(embedded_field) => {
                verify_document_embed(app, embedded_field.target)?;
            }
            app::FieldTy::BelongsTo(_) | app::FieldTy::Has(_) | app::FieldTy::Via(_) => {
                return Err(crate::Error::unsupported_feature(format!(
                    "embedded struct `{}` field `{name}` is a relation, which is \
                     not supported inside a #[document] field",
                    embedded.name
                )));
            }
        }
    }

    Ok(())
}

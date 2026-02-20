use super::BuildSchema;
use crate::{
    driver,
    schema::{
        app::{self, FieldId, Model},
        db::{self, ColumnId, IndexId, Table, TableId},
        mapping::{self, Mapping, TableToModel},
        Name,
    },
    stmt::{self},
};

struct BuildTableFromModels<'a> {
    /// Application schema (for looking up model definitions)
    app: &'a app::Schema,

    /// Database-specific capabilities
    db: &'a driver::Capability,

    /// The table being built from the set of models
    table: &'a mut Table,

    /// Schema mapping
    mapping: &'a mut Mapping,

    /// When true, column names should be prefixed with their associated model
    /// names
    prefix_table_names: bool,
}

/// Computes a model's maping
struct BuildMapping<'a> {
    app: &'a app::Schema,
    table: &'a mut Table,
    mapping: &'a mut mapping::Model,
    lowering_columns: Vec<ColumnId>,
    model_to_table: Vec<stmt::Expr>,
    model_pk_to_table: Vec<stmt::Expr>,
    table_to_model: Vec<stmt::Expr>,
}

impl BuildSchema<'_> {
    pub(super) fn build_table_stub_for_model(&mut self, model: &Model) -> TableId {
        let table_name = match &model.kind {
            app::ModelKind::Root(root) => root.table_name.as_ref(),
            app::ModelKind::Embedded => {
                panic!("build_table_stub_for_model called on embedded model")
            }
        };

        if let Some(table_name) = table_name {
            let table_name = self.prefix_table_name(table_name);

            if !self.table_lookup.contains_key(&table_name) {
                let id = self.register_table(&table_name);
                self.tables.push(Table::new(id, table_name.clone()));
            }

            *self.table_lookup.get(&table_name).unwrap()
        } else {
            let name = self.table_name_from_model(&model.name);
            let id = self.register_table(&name);

            self.tables.push(Table::new(id, name));
            id
        }
    }

    pub(super) fn build_tables_from_models(&mut self, app: &app::Schema, db: &driver::Capability) {
        for table in &mut self.tables {
            let models = app
                .models()
                .filter(|model| model.is_root())
                .filter(|model| self.mapping.model(model.id).table == table.id)
                .collect::<Vec<_>>();

            assert!(
                models.len() == 1,
                "TODO: handle mapping many models to one table"
            );

            BuildTableFromModels {
                app,
                db,
                table,
                mapping: &mut self.mapping,
                prefix_table_names: models.len() > 1,
            }
            .build(models[0]);
        }
    }

    pub(super) fn register_table(&mut self, name: impl AsRef<str>) -> TableId {
        assert!(!self.table_lookup.contains_key(name.as_ref()));
        let id = TableId(self.table_lookup.len());
        self.table_lookup.insert(name.as_ref().to_string(), id);
        id
    }

    fn table_name_from_model(&self, model_name: &Name) -> String {
        let base = std_util::str::pluralize(&model_name.snake_case());
        self.prefix_table_name(&base)
    }

    fn prefix_table_name(&self, name: &str) -> String {
        if let Some(prefix) = &self.builder.table_name_prefix {
            format!("{prefix}{name}")
        } else {
            name.to_string()
        }
    }
}

impl BuildTableFromModels<'_> {
    fn build(&mut self, model: &Model) {
        // Populate the rest of the columns
        self.map_model_fields(model);

        self.update_index_names();
    }

    fn map_model_fields(&mut self, model: &Model) {
        let schema_prefix = if self.prefix_table_names {
            Some(model.name.snake_case())
        } else {
            None
        };

        self.populate_columns(model, schema_prefix.as_deref(), None);

        BuildMapping {
            app: self.app,
            table: self.table,
            mapping: self.mapping.model_mut(model),
            lowering_columns: vec![],
            model_to_table: vec![],
            model_pk_to_table: vec![],
            table_to_model: vec![],
        }
        .build_mapping(model);

        self.populate_model_indices(model);
    }

    fn populate_model_indices(&mut self, model: &Model) {
        for model_index in &model.indices {
            let mut index = db::Index {
                id: IndexId {
                    table: self.table.id,
                    index: self.table.indices.len(),
                },
                name: String::new(),
                on: self.table.id,
                columns: vec![],
                unique: model_index.unique,
                primary_key: model_index.primary_key,
            };

            for index_field in &model_index.fields {
                let column = self.mapping.model(model.id).fields[index_field.field.index]
                    .as_primitive()
                    .unwrap()
                    .column;

                match &model.fields[index_field.field.index].ty {
                    app::FieldTy::Primitive(_) => index.columns.push(db::IndexColumn {
                        column,
                        op: index_field.op,
                        scope: index_field.scope,
                    }),
                    app::FieldTy::Embedded(_) => todo!("embedded field indexing"),
                    app::FieldTy::BelongsTo(_) => todo!(),
                    app::FieldTy::HasMany(_) => todo!(),
                    app::FieldTy::HasOne(_) => todo!(),
                }

                if model_index.primary_key {
                    self.table.primary_key.columns.push(column);
                }
            }

            self.table.indices.push(index);
        }
    }

    fn create_column(
        &mut self,
        column_name: String,
        field: &app::Field,
        primitive: &app::FieldPrimitive,
    ) {
        let auto_increment = field.is_auto_increment();
        let storage_ty = db::Type::from_app(
            &primitive.ty,
            primitive.storage_ty.as_ref(),
            &self.db.storage_types,
        )
        .expect("unsupported storage type");

        let column = db::Column {
            id: ColumnId {
                table: self.table.id,
                index: self.table.columns.len(),
            },
            name: column_name,
            ty: storage_ty.bridge_type(&primitive.ty),
            storage_ty,
            nullable: field.nullable,
            primary_key: false,
            auto_increment: auto_increment && self.db.auto_increment,
        };

        self.table.columns.push(column);
    }

    /// Creates database columns for all primitive fields in a model, recursing
    /// into embedded structs. Relations are skipped (they have no columns).
    fn populate_columns(
        &mut self,
        model: &Model,
        schema_prefix: Option<&str>,
        embed_prefix: Option<&str>,
    ) {
        for field in &model.fields {
            match &field.ty {
                app::FieldTy::Primitive(primitive) => {
                    let column_name = format_column_name(field, schema_prefix, embed_prefix);
                    self.create_column(column_name, field, primitive);
                }
                app::FieldTy::Embedded(embedded) => {
                    // schema_prefix stays separate and is not folded into the accumulated
                    // embed_prefix — it is only applied at the final format_column_name call.
                    let nested_embed_prefix = format_column_name(field, None, embed_prefix);
                    let nested_model = self.app.model(embedded.target);
                    self.populate_columns(nested_model, schema_prefix, Some(&nested_embed_prefix));
                }
                app::FieldTy::BelongsTo(_) | app::FieldTy::HasMany(_) | app::FieldTy::HasOne(_) => {
                }
            }
        }
    }

    fn update_index_names(&mut self) {
        for index in &mut self.table.indices {
            index.name = format!("index_{}_by", self.table.name);

            for (i, index_column) in index.columns.iter().enumerate() {
                let column = &self.table.columns[index_column.column.index];

                if i > 0 {
                    index.name.push_str("_and");
                }

                index.name.push('_');
                index.name.push_str(&column.name);
            }
        }
    }
}

impl BuildMapping<'_> {
    fn build_mapping(mut self, model: &Model) {
        // Build all field mappings in a single unified pass
        let fields = self.build_field_mappings(model);

        assert!(!self.model_to_table.is_empty());
        assert_eq!(self.model_to_table.len(), self.lowering_columns.len());

        // Iterate fields again (including PK fields) and build the table -> model map.
        for (field_index, field) in model.fields.iter().enumerate() {
            match &field.ty {
                app::FieldTy::Primitive(primitive) => {
                    let column_id = fields[field_index].as_primitive().unwrap().column;
                    let expr = self.map_table_column_to_model(column_id, primitive);
                    self.table_to_model.push(expr);
                }
                app::FieldTy::Embedded(_embedded) => {
                    // Use the mapping information we just built
                    let field_mapping = &fields[field_index];
                    let embedded_mapping = field_mapping
                        .as_embedded()
                        .expect("embedded field should have embedded mapping");
                    let expr = self.map_embedded_to_model_from_mapping(embedded_mapping);
                    self.table_to_model.push(expr);
                }
                app::FieldTy::BelongsTo(_) | app::FieldTy::HasMany(_) | app::FieldTy::HasOne(_) => {
                    self.table_to_model.push(stmt::Value::Null.into());
                }
            }
        }

        // Build the PK lowering
        for pk_field in &self.table.primary_key.columns {
            // Find the column's position in the mapping
            let index = self
                .lowering_columns
                .iter()
                .position(|column_id| column_id == pk_field)
                .unwrap();

            assert!(
                index < self.model_to_table.len(),
                "column={:#?}; index={}; lowering_columns={:#?}; mapping={:#?}",
                pk_field,
                index,
                self.lowering_columns,
                self.model_to_table
            );

            let expr = self.model_to_table[index].map_projections(|projection| {
                let [step, ..] = &projection[..] else {
                    todo!(
                        "projection={:#?}; mapping={:#?}",
                        projection,
                        self.model_to_table
                    )
                };

                let primary_key = model
                    .primary_key()
                    .expect("primary key required for model_pk_to_table");

                for (i, field_id) in primary_key.fields.iter().enumerate() {
                    if field_id.index == *step {
                        let mut p = projection.clone();
                        p[0] = i;

                        return p;
                    }
                }

                todo!(
                    "boom; projection={:?}; mapping={:#?}; PK={:#?}",
                    projection,
                    self.model_to_table,
                    primary_key
                );
            });

            self.model_pk_to_table.push(expr);
        }

        self.mapping.fields = fields;
        self.mapping.columns = self.lowering_columns;
        self.mapping.model_to_table = stmt::ExprRecord::from_vec(self.model_to_table);
        self.mapping.table_to_model =
            TableToModel::new(stmt::ExprRecord::from_vec(self.table_to_model));
        self.mapping.model_pk_to_table = if self.model_pk_to_table.len() == 1 {
            let expr = self.model_pk_to_table.into_iter().next().unwrap();
            debug_assert!(expr.is_field() || expr.is_cast(), "expr={expr:#?}");
            expr
        } else {
            stmt::ExprRecord::from_vec(self.model_pk_to_table).into()
        };
    }

    /// Builds field mappings for all fields in the model.
    ///
    /// This is a thin wrapper that calls the unified recursive function with
    /// root-level context (empty prefix, identity projection).
    fn build_field_mappings(&mut self, model: &Model) -> Vec<mapping::Field> {
        let mut next_bit = 0;
        self.map_fields_recursive(
            &model.fields,
            None,
            None,
            stmt::Projection::identity(),
            &mut next_bit,
        )
    }

    /// Unified recursive function that builds field mappings for any list of fields.
    ///
    /// Handles both root-level and embedded fields uniformly by using context variables:
    /// - For root fields: prefix="", source_field_id=None, base_projection=identity
    /// - For embedded fields: prefix="address_city", source_field_id=Some(User.address), base_projection=[1,1]
    ///
    /// The differences in column naming and expression building are extracted as
    /// simple conditionals based on these context variables.
    ///
    /// `next_bit` is a monotonically increasing counter that assigns each
    /// primitive field a unique bit index in the model's field mask space.
    fn map_fields_recursive(
        &mut self,
        fields: &[app::Field],
        prefix: Option<&str>,
        source_field_id: Option<FieldId>,
        base_projection: stmt::Projection,
        next_bit: &mut usize,
    ) -> Vec<mapping::Field> {
        fields
            .iter()
            .enumerate()
            .map(|(field_index, field)| {
                match &field.ty {
                    app::FieldTy::Primitive(primitive) => {
                        let column_name = format_column_name(field, None, prefix);

                        let column_id = self
                            .table
                            .columns
                            .iter()
                            .find(|col| col.name == column_name)
                            .map(|col| col.id)
                            .expect("column should exist for primitive field");

                        // Expression: root primitives use ref(field.id), embedded uses project(ref(source), projection)
                        let expr = if let Some(source) = source_field_id {
                            // Embedded primitive: project from source field through accumulated path
                            let base = stmt::Expr::ref_self_field(source);
                            let mut projection = base_projection.clone();
                            projection.push(field_index);
                            stmt::Expr::project(base, projection)
                        } else {
                            // Root primitive: reference the field directly
                            stmt::Expr::ref_self_field(field.id)
                        };

                        let lowering = self.encode_column(column_id, &primitive.ty, expr);
                        let lowering_index = self.model_to_table.len();

                        self.lowering_columns.push(column_id);
                        self.model_to_table.push(lowering);

                        // Assign this primitive its unique bit in the field mask space.
                        let bit = *next_bit;
                        *next_bit += 1;

                        // sub_projection is the path from the root embedded field
                        // to this primitive: base_projection + [field_index] for
                        // embedded primitives, identity for root-level primitives.
                        let sub_projection = if source_field_id.is_some() {
                            let mut proj = base_projection.clone();
                            proj.push(field_index);
                            proj
                        } else {
                            stmt::Projection::identity()
                        };

                        mapping::Field::Primitive(mapping::FieldPrimitive {
                            column: column_id,
                            lowering: lowering_index,
                            field_mask: stmt::PathFieldSet::from_iter([bit]),
                            sub_projection,
                        })
                    }
                    app::FieldTy::Embedded(embedded) => {
                        let nested_prefix = format_column_name(field, None, prefix);

                        // Nested source: root embedded uses field.id, nested embedded keeps source_field_id
                        let nested_source = source_field_id.or(Some(field.id));

                        // Nested projection: reset for root embedded, extend for nested embedded
                        let nested_projection = if source_field_id.is_none() {
                            // Root embedded: start fresh with identity projection
                            stmt::Projection::identity()
                        } else {
                            // Nested embedded: extend the accumulated projection
                            let mut proj = base_projection.clone();
                            proj.push(field_index);
                            proj
                        };

                        // Recurse with a shared bit counter so all nested primitives
                        // get globally unique bits within the model's field mask space.
                        let embedded_model = self.app.model(embedded.target);
                        let nested_fields = self.map_fields_recursive(
                            &embedded_model.fields,
                            Some(&nested_prefix),
                            nested_source,
                            nested_projection.clone(),
                            next_bit,
                        );

                        // Derive the columns map from the nested fields
                        let columns: indexmap::IndexMap<ColumnId, usize> = nested_fields
                            .iter()
                            .flat_map(|field| field.columns())
                            .collect();

                        // The embedded field's mask is the union of all nested
                        // primitive masks, giving full coverage of the embedded struct.
                        let field_mask = nested_fields
                            .iter()
                            .fold(stmt::PathFieldSet::new(), |acc, f| acc | f.field_mask());

                        mapping::Field::Embedded(mapping::FieldEmbedded {
                            fields: nested_fields,
                            columns,
                            field_mask,
                            sub_projection: nested_projection,
                        })
                    }
                    app::FieldTy::BelongsTo(_)
                    | app::FieldTy::HasMany(_)
                    | app::FieldTy::HasOne(_) => {
                        let bit = *next_bit;
                        *next_bit += 1;
                        mapping::Field::Relation(mapping::FieldRelation {
                            field_mask: stmt::PathFieldSet::from_iter([bit]),
                        })
                    }
                }
            })
            .collect()
    }

    fn encode_column(
        &self,
        column_id: ColumnId,
        ty: &stmt::Type,
        expr: impl Into<stmt::Expr>,
    ) -> stmt::Expr {
        let expr = expr.into();
        let column = self.table.column(column_id);

        assert_ne!(stmt::Type::Null, *ty);

        match &column.ty {
            column_ty if column_ty == ty => expr,
            // If the types do not match, attempt casting as a fallback.
            _ => stmt::Expr::cast(expr, &column.ty),
        }
    }

    /// Maps table columns to model field expressions during query lowering.
    ///
    /// Called during query planning to replace model field references with the
    /// appropriate table column expressions. Handles type conversions between
    /// table storage and model types.
    fn map_table_column_to_model(
        &mut self,
        column_id: ColumnId,
        primitive: &app::FieldPrimitive,
    ) -> stmt::Expr {
        let column = self.table.column(column_id);

        // NOTE: nesting and table are stubs here (though often the actual values).
        // The engine must substitute these with the actual TableRef index in the query's TableSource.
        let expr_column = stmt::Expr::column(stmt::ExprColumn {
            nesting: 0,
            table: 0,
            column: column_id.index,
        });

        match &column.ty {
            c_ty if *c_ty == primitive.ty => expr_column,
            // If the types do not match, attempt casting as a fallback.
            _ => stmt::Expr::cast(expr_column, &primitive.ty),
        }
    }

    /// Maps flattened table columns to an embedded struct expression.
    ///
    /// Constructs an ExprRecord that builds the embedded struct from multiple
    /// columns. For example, if Address has fields (street, city) stored as
    /// columns (address_street, address_city), this creates:
    /// record(column[1], column[2])
    ///
    /// Uses the field mapping information that was already built during `map_embedded`,
    /// avoiding the need to recompute column names and perform lookups.
    fn map_embedded_to_model_from_mapping(
        &self,
        embedded_mapping: &mapping::FieldEmbedded,
    ) -> stmt::Expr {
        let mut field_exprs = Vec::new();

        for field in &embedded_mapping.fields {
            match field {
                mapping::Field::Primitive(primitive_mapping) => {
                    let column_id = primitive_mapping.column;

                    // Create a column reference expression
                    let expr_column = stmt::Expr::column(stmt::ExprColumn {
                        nesting: 0,
                        table: 0,
                        column: column_id.index,
                    });

                    field_exprs.push(expr_column);
                }
                mapping::Field::Embedded(nested_embedded_mapping) => {
                    // Recursively build the nested record expression
                    let nested_expr =
                        self.map_embedded_to_model_from_mapping(nested_embedded_mapping);
                    field_exprs.push(nested_expr);
                }
                mapping::Field::Relation(_) => {
                    panic!("relations not allowed in embedded types")
                }
            }
        }

        // Build a record expression from the field expressions
        stmt::Expr::record(field_exprs)
    }
}

/// Formats a database column name from its components.
///
/// - `schema_prefix`: model name prefix used when multiple models share one table
///   (separated from the rest with `__`)
/// - `embed_prefix`: accumulated field path for embedded structs
///   (separated with `_`)
///
/// Examples:
/// - `format_column_name(field, None, None)` → `"street"`
/// - `format_column_name(field, None, Some("address"))` → `"address_street"`
/// - `format_column_name(field, Some("user"), None)` → `"user__street"`
/// - `format_column_name(field, Some("user"), Some("address"))` → `"user__address_street"`
fn format_column_name(
    field: &app::Field,
    schema_prefix: Option<&str>,
    embed_prefix: Option<&str>,
) -> String {
    let field_name = field.name.storage_name();
    match (schema_prefix, embed_prefix) {
        (None, None) => field_name.to_owned(),
        (Some(sp), None) => format!("{sp}__{field_name}"),
        (None, Some(ep)) => format!("{ep}_{field_name}"),
        (Some(sp), Some(ep)) => format!("{sp}__{ep}_{field_name}"),
    }
}

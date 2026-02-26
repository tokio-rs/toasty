use super::BuildSchema;
use crate::{
    driver,
    schema::{
        app::{self, FieldId, Model, ModelRoot},
        db::{self, ColumnId, IndexId, Table, TableId},
        mapping::{self, Mapping, TableToModel},
        Name,
    },
    stmt::{self, ExprArg, Input, Projection},
};

/// An `Input` that replaces `Arg(0)` with a single concrete expression.
///
/// Used by `MapField::field_expr` to substitute the raw field expression into
/// `field_expr_base`, which may contain `Expr::arg(0)` as a placeholder.
struct SingleArgInput(stmt::Expr);

impl Input for SingleArgInput {
    fn resolve_arg(&mut self, expr_arg: &ExprArg, projection: &Projection) -> Option<stmt::Expr> {
        if expr_arg.position == 0 {
            let expr = self.0.clone();
            Some(if projection.is_identity() {
                expr
            } else {
                stmt::Expr::project(expr, projection.clone())
            })
        } else {
            None
        }
    }
}

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

/// Computes a model's mapping, creating table columns and mapping expressions
/// in a single recursive pass over the model's fields.
///
/// Holds state that persists across the entire mapping process: the shared
/// mutable accumulators (columns, lowering expressions, bit counter) plus
/// references to the table and schema. The recursive field-mapping logic lives
/// on [`MapField`], which borrows `BuildMapping` and carries per-level context.
struct BuildMapping<'a> {
    app: &'a app::Schema,
    db: &'a driver::Capability,
    table: &'a mut Table,
    mapping: &'a mut mapping::Model,
    /// Model-name prefix used when multiple models share one table, separated
    /// from the rest of the column name with `__`. None for single-model tables.
    schema_prefix: Option<String>,
    next_bit: usize,
    lowering_columns: Vec<ColumnId>,
    model_to_table: Vec<stmt::Expr>,
    model_pk_to_table: Vec<stmt::Expr>,
    table_to_model: Vec<stmt::Expr>,
}

/// Per-level state for the recursive `map_field*` methods.
///
/// Analogous to `LowerStatement` in `lower.rs`: `MapField` holds context that
/// may change between recursive calls, while [`BuildMapping`] holds the shared
/// mutable accumulators (columns, lowering expressions, bit counter) that
/// persist across the entire mapping process.
struct MapField<'a, 'b> {
    /// State shared across the entire mapping process.
    build: &'a mut BuildMapping<'b>,

    /// Accumulated embed-prefix components (without schema_prefix), pushed on
    /// entry to each nested field and popped on exit.
    ///
    /// Final column names join these with `_`, append the field name, then
    /// prepend the schema prefix (if any) with `__`. Keeping components
    /// separate ensures schema_prefix is applied exactly once.
    prefix: Vec<String>,

    /// When true, columns are created nullable regardless of the field's own
    /// nullability. Set while processing fields that belong to an enum variant,
    /// since only the active variant's columns are populated.
    in_enum_variant: bool,

    /// The root model field whose value is projected to reach the current
    /// nesting level. `None` at the top level (each field is its own source).
    /// Once set when entering the first embedded struct, it stays the same
    /// through all deeper nesting levels.
    source_field_id: Option<FieldId>,

    /// The projection from `source_field_id` down to the current nesting level.
    /// Identity at the top level and when first entering an embedded struct;
    /// extended by one step per additional level of nesting.
    base_projection: stmt::Projection,

    /// A template expression with `Expr::arg(0)` as a placeholder for the raw
    /// field expression. `field_expr` substitutes the raw field expression into
    /// this template before returning. `None` means identity (no wrapping).
    ///
    /// Used by variant-specific `MapField` instances to automatically wrap
    /// field expressions in the discriminant match guard.
    field_expr_base: Option<stmt::Expr>,

    /// Added to `field_index` in `field_expr` when building the projection.
    ///
    /// A data-carrying enum value is represented as `Record([disc, vf0, vf1,
    /// ...])`, so position 0 is the discriminant and variant fields start at
    /// position 1. The variant field loop uses 0-based `local_idx`, so without
    /// an offset `field_expr(vf, 0)` would project to position 0 (the
    /// discriminant) instead of position 1 (the first variant field).
    ///
    /// Set to 1 on variant-level `MapField` instances (via `for_variant`) so
    /// that `field_expr(vf, local_idx)` projects to `local_idx + 1`. Zero
    /// everywhere else.
    field_index_offset: usize,
}

impl BuildSchema<'_> {
    pub(super) fn build_table_stub_for_model(&mut self, model: &ModelRoot) -> TableId {
        if let Some(table_name) = model.table_name.as_ref() {
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
                .filter(|model| self.mapping.model(model.id()).table == table.id)
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
        self.map_model_fields(model);
        self.update_index_names();
    }

    fn map_model_fields(&mut self, model: &Model) {
        let root = model.expect_root();
        let schema_prefix = if self.prefix_table_names {
            Some(model.name().snake_case())
        } else {
            None
        };

        BuildMapping {
            app: self.app,
            db: self.db,
            table: self.table,
            mapping: self.mapping.model_mut(model),
            schema_prefix,
            next_bit: 0,
            lowering_columns: vec![],
            model_to_table: vec![],
            model_pk_to_table: vec![],
            table_to_model: vec![],
        }
        .build_mapping(root);

        self.populate_model_indices(model.id(), root);
    }

    fn populate_model_indices(&mut self, model_id: app::ModelId, root: &ModelRoot) {
        for model_index in &root.indices {
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
                let column = self.mapping.model(model_id).fields[index_field.field.index]
                    .as_primitive()
                    .unwrap()
                    .column;

                match &root.fields[index_field.field.index].ty {
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
    fn build_mapping(mut self, model: &ModelRoot) {
        let fields = MapField::new(&mut self).map_fields(&model.fields);

        assert!(!self.model_to_table.is_empty());
        assert_eq!(self.model_to_table.len(), self.lowering_columns.len());

        self.build_table_to_model(model, &fields);
        self.build_pk_lowering(model);

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

    fn next_bit(&mut self) -> usize {
        let bit = self.next_bit;
        self.next_bit += 1;
        bit
    }

    fn build_table_to_model(&mut self, model: &ModelRoot, mapping: &[mapping::Field]) {
        for (index, field) in model.fields.iter().enumerate() {
            let expr = self.build_table_to_model_field(field, &mapping[index]);
            self.table_to_model.push(expr);
        }
    }

    /// Builds the `table_to_model` expression for an embedded enum field.
    ///
    /// For unit-only enums the discriminant column reference suffices.
    /// For mixed/data-carrying enums a `Match` expression dispatches on the
    /// discriminant: unit arms return the discriminant directly, data arms
    /// return `Record([disc, field1, ...])` matching the shape expected by
    /// `Primitive::load`.
    fn build_table_to_model_field_enum(
        &self,
        model: &app::EmbeddedEnum,
        mapping: &mapping::FieldEnum,
    ) -> stmt::Expr {
        let disc_col_ref = stmt::Expr::column(stmt::ExprColumn {
            nesting: 0,
            table: 0,
            column: mapping.disc_column.index,
        });

        if !model.has_data_variants() {
            return disc_col_ref;
        }

        let mut arms = Vec::new();

        for (variant, mapping) in model.variants.iter().zip(&mapping.variants) {
            let arm_expr = if variant.fields.is_empty() {
                disc_col_ref.clone()
            } else {
                let mut record_elems = vec![disc_col_ref.clone()];

                for (local_idx, field) in variant.fields.iter().enumerate() {
                    let expr = self.build_table_to_model_field(field, &mapping.fields[local_idx]);
                    record_elems.push(expr);
                }
                stmt::Expr::record(record_elems)
            };
            arms.push(stmt::MatchArm {
                pattern: stmt::Value::I64(variant.discriminant),
                expr: arm_expr,
            });
        }
        stmt::Expr::match_expr(disc_col_ref, arms, stmt::Expr::null())
    }

    fn build_pk_lowering(&mut self, model: &ModelRoot) {
        for pk_field in &self.table.primary_key.columns {
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

                for (i, field_id) in model.primary_key.fields.iter().enumerate() {
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
                    model.primary_key
                );
            });

            self.model_pk_to_table.push(expr);
        }
    }

    /// Encodes `expr` for `column_id`, appends the result to `model_to_table`,
    /// records the column in `lowering_columns`, and returns the lowering index.
    fn push_lowering(
        &mut self,
        column_id: ColumnId,
        ty: &stmt::Type,
        expr: impl Into<stmt::Expr>,
    ) -> usize {
        let lowering_expr = self.encode_column(column_id, ty, expr);
        let lowering_index = self.model_to_table.len();
        self.lowering_columns.push(column_id);
        self.model_to_table.push(lowering_expr);
        lowering_index
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
        &self,
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

    fn build_table_to_model_field_struct(
        &self,
        model: &app::EmbeddedStruct,
        mapping: &mapping::FieldStruct,
    ) -> stmt::Expr {
        let exprs: Vec<_> = model
            .fields
            .iter()
            .enumerate()
            .map(|(index, field)| self.build_table_to_model_field(field, &mapping.fields[index]))
            .collect();
        stmt::Expr::record(exprs)
    }

    fn build_table_to_model_field(
        &self,
        field: &app::Field,
        mapping: &mapping::Field,
    ) -> stmt::Expr {
        match &field.ty {
            app::FieldTy::Primitive(primitive) => {
                let column_id = mapping.as_primitive().unwrap().column;
                self.map_table_column_to_model(column_id, primitive)
            }
            app::FieldTy::Embedded(embedded) => match self.app.model(embedded.target) {
                app::Model::EmbeddedEnum(embedded) => {
                    let mapping = mapping
                        .as_enum()
                        .expect("embedded enum field should have enum mapping");
                    self.build_table_to_model_field_enum(embedded, mapping)
                }
                app::Model::EmbeddedStruct(embedded) => {
                    let mapping = mapping
                        .as_struct()
                        .expect("embedded struct field should have struct mapping");
                    self.build_table_to_model_field_struct(embedded, mapping)
                }
                _ => unreachable!("invalid schema"),
            },
            app::FieldTy::BelongsTo(_) | app::FieldTy::HasMany(_) | app::FieldTy::HasOne(_) => {
                stmt::Value::Null.into()
            }
        }
    }
}

impl<'a, 'b> MapField<'a, 'b> {
    fn new(build: &'a mut BuildMapping<'b>) -> Self {
        MapField {
            build,
            prefix: vec![],
            in_enum_variant: false,
            source_field_id: None,
            base_projection: stmt::Projection::identity(),
            field_expr_base: None,
            field_index_offset: 0,
        }
    }

    /// Builds the final database column name for `field` at the current nesting level.
    ///
    /// Joins `self.prefix` components with `_`, appends the field name, then
    /// prepends `schema_prefix` (if any) with `__`. Because `schema_prefix` is
    /// applied here — never stored in `self.prefix` — it is always applied
    /// exactly once regardless of nesting depth.
    fn column_name(&self, field: &app::Field) -> String {
        let field_name = field.name.storage_name();
        let embed = if self.prefix.is_empty() {
            field_name.to_owned()
        } else {
            format!("{}_{field_name}", self.prefix.join("_"))
        };
        match self.build.schema_prefix.as_deref() {
            None => embed,
            Some(sp) => format!("{sp}__{embed}"),
        }
    }

    /// Creates a column for `field` using `primitive` for the storage type.
    ///
    /// Derives the column name from `self.column_name(field)`, nullability from
    /// `field.nullable || self.in_enum_variant`, and auto-increment from
    /// `field.is_auto_increment()`.
    fn create_column(&mut self, field: &app::Field, primitive: &app::FieldPrimitive) -> ColumnId {
        let storage_ty = db::Type::from_app(
            &primitive.ty,
            primitive.storage_ty.as_ref(),
            &self.build.db.storage_types,
        )
        .expect("unsupported storage type");

        let id = ColumnId {
            table: self.build.table.id,
            index: self.build.table.columns.len(),
        };

        self.build.table.columns.push(db::Column {
            id,
            name: self.column_name(field),
            ty: storage_ty.bridge_type(&primitive.ty),
            storage_ty,
            nullable: field.nullable || self.in_enum_variant,
            primary_key: false,
            auto_increment: field.is_auto_increment() && self.build.db.auto_increment,
        });

        id
    }

    /// Creates a variant-specific child `MapField`.
    ///
    /// Sets `source_field_id` and `base_projection` so that `field_expr` on
    /// the child projects from the enum field (mirroring `for_struct`), sets
    /// `in_enum_variant = true`, and installs a `field_expr_base` of
    /// `match_expr(disc_proj, [arm(discriminant, Expr::arg(0))], null())` so
    /// that every `field_expr` call is automatically wrapped in the
    /// discriminant check. `field_index_offset` is set to 1 to skip the
    /// discriminant at record position 0.
    fn for_variant(
        &mut self,
        field: &app::Field,
        field_index: usize,
        disc_proj: stmt::Expr,
        discriminant: i64,
    ) -> MapField<'_, 'b> {
        let source_field_id = self.source_field_id.or(Some(field.id));
        let base_projection = if self.source_field_id.is_none() {
            stmt::Projection::identity()
        } else {
            let mut proj = self.base_projection.clone();
            proj.push(field_index);
            proj
        };
        let field_expr_base = stmt::Expr::match_expr(
            disc_proj,
            vec![stmt::MatchArm {
                pattern: stmt::Value::I64(discriminant),
                expr: stmt::Expr::arg(0),
            }],
            stmt::Expr::null(),
        );
        let mut child = self.with_prefix(field.name.storage_name());
        child.in_enum_variant = true;
        child.source_field_id = source_field_id;
        child.base_projection = base_projection;
        child.field_expr_base = Some(field_expr_base);
        child.field_index_offset = 1;
        child
    }

    /// Creates a child `MapField` for recursing into an embedded field.
    ///
    /// The child inherits the current prefix extended by `name` and inherits
    /// `in_enum_variant`, `source_field_id`, and `base_projection` unchanged.
    /// `field_expr_base` is always reset to `None` (identity) for the child.
    /// Used when entering struct fields so that sub-field columns are named
    /// `{..prefix..}_{name}_{sub_field}`.
    fn with_prefix(&mut self, name: &str) -> MapField<'_, 'b> {
        let mut prefix = self.prefix.clone();
        prefix.push(name.to_owned());
        MapField {
            build: self.build,
            prefix,
            in_enum_variant: self.in_enum_variant,
            source_field_id: self.source_field_id,
            base_projection: self.base_projection.clone(),
            field_expr_base: None,
            field_index_offset: 0,
        }
    }

    /// Creates a child `MapField` for recursing into an embedded struct field.
    ///
    /// Updates `source_field_id` and `base_projection` to reflect the new
    /// nesting level: if entering the first embedded level, sets the source
    /// to this field with an identity projection; at deeper levels, keeps the
    /// original source and extends the projection by `field_index`.
    fn for_struct(&mut self, field: &app::Field, field_index: usize) -> MapField<'_, 'b> {
        // Compute new values before borrowing self mutably for with_prefix.
        let source_field_id = self.source_field_id.or(Some(field.id));
        let base_projection = if self.source_field_id.is_none() {
            stmt::Projection::identity()
        } else {
            let mut proj = self.base_projection.clone();
            proj.push(field_index);
            proj
        };
        let mut child = self.with_prefix(field.name.storage_name());
        child.source_field_id = source_field_id;
        child.base_projection = base_projection;
        child
    }

    /// Returns the sub-projection from the root source field to a field at
    /// `field_index` within the current nesting level.
    ///
    /// At the top level (`source_field_id` is `None`) the field is its own
    /// root, so the sub-projection is identity. Inside an embedded type the
    /// sub-projection is `base_projection` extended by `field_index`.
    fn sub_projection(&self, field_index: usize) -> stmt::Projection {
        if self.source_field_id.is_some() {
            let mut proj = self.base_projection.clone();
            proj.push(field_index);
            proj
        } else {
            stmt::Projection::identity()
        }
    }

    /// Builds the lowering expression for a field at the current nesting level.
    ///
    /// At the top level (`source_field_id` is `None`) each field references
    /// itself directly. Inside an embedded struct the expression projects from
    /// the root source field through `base_projection` extended by `field_index`.
    ///
    /// If `field_expr_base` is set, the raw expression is substituted for
    /// `Expr::arg(0)` in the template before returning.
    fn field_expr(&self, field: &app::Field, field_index: usize) -> stmt::Expr {
        let raw = if let Some(source) = self.source_field_id {
            let base = stmt::Expr::ref_self_field(source);
            let mut projection = self.base_projection.clone();
            projection.push(field_index + self.field_index_offset);
            stmt::Expr::project(base, projection)
        } else {
            stmt::Expr::ref_self_field(field.id)
        };

        if let Some(base) = &self.field_expr_base {
            let mut result = base.clone();
            result.substitute(SingleArgInput(raw));
            result
        } else {
            raw
        }
    }

    fn map_fields(&mut self, fields: &[app::Field]) -> Vec<mapping::Field> {
        fields
            .iter()
            .enumerate()
            .map(|(index, field)| self.map_field(index, field))
            .collect()
    }

    fn map_field(&mut self, index: usize, field: &app::Field) -> mapping::Field {
        match &field.ty {
            app::FieldTy::Primitive(primitive) => self.map_field_primitive(index, field, primitive),
            app::FieldTy::Embedded(embedded) => {
                let embedded_model = self.build.app.model(embedded.target);
                if let app::Model::EmbeddedEnum(embedded_enum) = embedded_model {
                    self.map_field_enum(index, field, embedded_enum)
                } else {
                    self.map_field_struct(index, field, embedded_model.expect_embedded_struct())
                }
            }
            app::FieldTy::BelongsTo(_) | app::FieldTy::HasMany(_) | app::FieldTy::HasOne(_) => {
                let bit = self.build.next_bit();
                mapping::Field::Relation(mapping::FieldRelation {
                    field_mask: stmt::PathFieldSet::from_iter([bit]),
                })
            }
        }
    }

    /// Creates the column and builds the mapping for a primitive field in one step.
    fn map_field_primitive(
        &mut self,
        field_index: usize,
        field: &app::Field,
        primitive: &app::FieldPrimitive,
    ) -> mapping::Field {
        let column_id = self.create_column(field, primitive);
        let expr = self.field_expr(field, field_index);
        let lowering_index = self.build.push_lowering(column_id, &primitive.ty, expr);
        let bit = self.build.next_bit();
        let sub_projection = self.sub_projection(field_index);

        mapping::Field::Primitive(mapping::FieldPrimitive {
            column: column_id,
            lowering: lowering_index,
            field_mask: stmt::PathFieldSet::from_iter([bit]),
            sub_projection,
        })
    }

    /// Creates the discriminant and variant-field columns, then builds the
    /// enum mapping — all in a single pass.
    fn map_field_enum(
        &mut self,
        field_index: usize,
        field: &app::Field,
        embedded_enum: &app::EmbeddedEnum,
    ) -> mapping::Field {
        // Create the discriminant column. It inherits nullability from the enum field.
        let disc_col_id = self.create_column(field, &embedded_enum.discriminant);
        let field_expr = self.field_expr(field, field_index);

        // For data-carrying enums the model value is Record([I64(disc), ...]),
        // so project [0] to extract the discriminant; for unit-only enums the
        // value IS the I64 discriminant directly.
        let disc_expr = if embedded_enum.has_data_variants() {
            stmt::Expr::project(field_expr.clone(), stmt::Projection::single(0))
        } else {
            field_expr.clone()
        };

        let lowering_index =
            self.build
                .push_lowering(disc_col_id, &embedded_enum.discriminant.ty, disc_expr);

        let bit = self.build.next_bit();

        let sub_projection = self.sub_projection(field_index);

        let disc_proj = stmt::Expr::project(field_expr.clone(), stmt::Projection::single(0));

        let variants = embedded_enum
            .variants
            .iter()
            .map(|variant| {
                let fields = self
                    .for_variant(field, field_index, disc_proj.clone(), variant.discriminant)
                    .map_variant(variant, &field_expr, &disc_proj, bit);
                mapping::EnumVariant {
                    discriminant: variant.discriminant,
                    fields,
                }
            })
            .collect();

        mapping::Field::Enum(mapping::FieldEnum {
            disc_column: disc_col_id,
            disc_lowering: lowering_index,
            variants,
            field_mask: stmt::PathFieldSet::from_iter([bit]),
            sub_projection,
        })
    }

    fn map_variant(
        &mut self,
        variant: &app::EnumVariant,
        field_expr: &stmt::Expr,
        disc_proj: &stmt::Expr,
        bit: usize,
    ) -> Vec<mapping::Field> {
        variant
            .fields
            .iter()
            .enumerate()
            .map(|(local_idx, vf)| match &vf.ty {
                app::FieldTy::Primitive(vf_primitive) => {
                    let vf_col_id = self.create_column(vf, vf_primitive);
                    let vf_lowering = self.build.push_lowering(
                        vf_col_id,
                        &vf_primitive.ty,
                        self.field_expr(vf, local_idx),
                    );
                    mapping::Field::Primitive(mapping::FieldPrimitive {
                        column: vf_col_id,
                        lowering: vf_lowering,
                        field_mask: stmt::PathFieldSet::from_iter([bit]),
                        sub_projection: stmt::Projection::single(local_idx + 1),
                    })
                }
                app::FieldTy::Embedded(embedded) => {
                    let embedded_model = self.build.app.model(embedded.target);
                    let embedded_struct = embedded_model.expect_embedded_struct();
                    self.with_prefix(vf.name.storage_name())
                        .map_variant_struct_field(
                            local_idx,
                            embedded_struct,
                            field_expr,
                            disc_proj,
                            variant.discriminant,
                            bit,
                        )
                }
                _ => panic!("unexpected field type in enum variant"),
            })
            .collect()
    }

    fn map_field_struct(
        &mut self,
        field_index: usize,
        field: &app::Field,
        embedded_struct: &app::EmbeddedStruct,
    ) -> mapping::Field {
        let sub_projection = self.sub_projection(field_index);

        let nested_fields = self
            .for_struct(field, field_index)
            .map_fields(&embedded_struct.fields);

        let columns: indexmap::IndexMap<ColumnId, usize> =
            nested_fields.iter().flat_map(|f| f.columns()).collect();
        let field_mask = nested_fields
            .iter()
            .fold(stmt::PathFieldSet::new(), |acc, f| acc | f.field_mask());

        mapping::Field::Struct(mapping::FieldStruct {
            fields: nested_fields,
            columns,
            field_mask,
            sub_projection,
        })
    }

    /// Expands a struct-typed field inside a data-carrying enum variant into
    /// its flattened column representation.
    ///
    /// Each primitive sub-field of the struct becomes a standalone nullable
    /// column; the caller is responsible for pushing the struct field's name
    /// onto `self.prefix` before calling so that `column_name` produces the
    /// correct `{enum_field}_{vf_name}_{sub_field}` names.
    fn map_variant_struct_field(
        &mut self,
        local_idx: usize,
        embedded_struct: &app::EmbeddedStruct,
        field_expr: &stmt::Expr,
        disc_proj: &stmt::Expr,
        discriminant: i64,
        bit: usize,
    ) -> mapping::Field {
        let mut sub_fields = Vec::new();

        for (sub_idx, sub_field) in embedded_struct.fields.iter().enumerate() {
            let app::FieldTy::Primitive(sub_primitive) = &sub_field.ty else {
                todo!("deeply nested structs in enum variants not yet supported");
            };

            let sub_col_id = self.create_column(sub_field, sub_primitive);

            // The enum value is Record([I64(disc), vf_0, vf_1, ...]).
            // The struct field at local_idx is at position local_idx + 1.
            // The sub-field at sub_idx is at position sub_idx within the struct record.
            let mut arm_proj = stmt::Projection::single(local_idx + 1);
            arm_proj.push(sub_idx);

            let arm = stmt::MatchArm {
                pattern: stmt::Value::I64(discriminant),
                expr: stmt::Expr::project(field_expr.clone(), arm_proj.clone()),
            };

            let sub_lowering = self.build.push_lowering(
                sub_col_id,
                &sub_primitive.ty,
                stmt::Expr::match_expr(disc_proj.clone(), vec![arm], stmt::Expr::null()),
            );

            sub_fields.push(mapping::Field::Primitive(mapping::FieldPrimitive {
                column: sub_col_id,
                lowering: sub_lowering,
                field_mask: stmt::PathFieldSet::from_iter([bit]),
                sub_projection: arm_proj,
            }));
        }

        let columns: indexmap::IndexMap<ColumnId, usize> =
            sub_fields.iter().flat_map(|f| f.columns()).collect();
        let field_mask = sub_fields
            .iter()
            .fold(stmt::PathFieldSet::new(), |acc, f| acc | f.field_mask());

        mapping::Field::Struct(mapping::FieldStruct {
            fields: sub_fields,
            columns,
            field_mask,
            sub_projection: stmt::Projection::single(local_idx + 1),
        })
    }
}

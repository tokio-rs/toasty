use super::BuildSchema;
use crate::{
    driver,
    schema::{
        app::{self, FieldId, Model, ModelRoot},
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

/// Computes a model's mapping, creating table columns and mapping expressions
/// in a single recursive pass over the model's fields.
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
    /// When true, columns are created nullable regardless of the field's own
    /// nullability. Set while processing fields that belong to an enum variant,
    /// since only the active variant's columns are populated.
    in_enum_variant: bool,
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
            in_enum_variant: false,
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
        let fields = self.map_fields(&model.fields, None, None, stmt::Projection::identity());

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

    /// Creates a database column, appends it to the table, and returns its ID.
    fn create_column(
        &mut self,
        column_name: String,
        primitive: &app::FieldPrimitive,
        nullable: bool,
        auto_increment: bool,
    ) -> ColumnId {
        let storage_ty = db::Type::from_app(
            &primitive.ty,
            primitive.storage_ty.as_ref(),
            &self.db.storage_types,
        )
        .expect("unsupported storage type");

        let id = ColumnId {
            table: self.table.id,
            index: self.table.columns.len(),
        };

        let column = db::Column {
            id,
            name: column_name,
            ty: storage_ty.bridge_type(&primitive.ty),
            storage_ty,
            nullable,
            primary_key: false,
            auto_increment: auto_increment && self.db.auto_increment,
        };

        self.table.columns.push(column);
        id
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

    fn map_fields(
        &mut self,
        fields: &[app::Field],
        prefix: Option<&str>,
        source_field_id: Option<FieldId>,
        base_projection: stmt::Projection,
    ) -> Vec<mapping::Field> {
        let mut mapping = Vec::with_capacity(fields.len());
        for (field_index, field) in fields.iter().enumerate() {
            mapping.push(self.map_field(
                field_index,
                field,
                prefix,
                source_field_id,
                &base_projection,
            ));
        }
        mapping
    }

    fn map_field(
        &mut self,
        field_index: usize,
        field: &app::Field,
        prefix: Option<&str>,
        source_field_id: Option<FieldId>,
        base_projection: &stmt::Projection,
    ) -> mapping::Field {
        match &field.ty {
            app::FieldTy::Primitive(primitive) => self.map_field_primitive(
                field_index,
                field,
                primitive,
                prefix,
                source_field_id,
                base_projection,
            ),
            app::FieldTy::Embedded(embedded) => {
                let embedded_model = self.app.model(embedded.target);
                if let app::Model::EmbeddedEnum(embedded_enum) = embedded_model {
                    self.map_field_enum(
                        field_index,
                        field,
                        embedded_enum,
                        prefix,
                        source_field_id,
                        base_projection,
                    )
                } else {
                    self.map_field_struct(
                        field_index,
                        field,
                        embedded_model.expect_embedded_struct(),
                        prefix,
                        source_field_id,
                        base_projection,
                    )
                }
            }
            app::FieldTy::BelongsTo(_) | app::FieldTy::HasMany(_) | app::FieldTy::HasOne(_) => {
                let bit = self.next_bit();
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
        prefix: Option<&str>,
        source_field_id: Option<FieldId>,
        base_projection: &stmt::Projection,
    ) -> mapping::Field {
        let column_name = format_column_name(field, self.schema_prefix.as_deref(), prefix);
        let column_id = self.create_column(
            column_name,
            primitive,
            field.nullable || self.in_enum_variant,
            field.is_auto_increment(),
        );

        let expr = field_expr(field, field_index, source_field_id, base_projection);

        let lowering = self.encode_column(column_id, &primitive.ty, expr);
        let lowering_index = self.model_to_table.len();
        self.lowering_columns.push(column_id);
        self.model_to_table.push(lowering);

        let bit = self.next_bit();

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

    /// Creates the discriminant and variant-field columns, then builds the
    /// enum mapping — all in a single pass.
    fn map_field_enum(
        &mut self,
        field_index: usize,
        field: &app::Field,
        embedded_enum: &app::EmbeddedEnum,
        prefix: Option<&str>,
        source_field_id: Option<FieldId>,
        base_projection: &stmt::Projection,
    ) -> mapping::Field {
        let disc_col_name = format_column_name(field, self.schema_prefix.as_deref(), prefix);

        // Create the discriminant column. It inherits nullability from the enum field.
        let disc_primitive = app::FieldPrimitive {
            ty: stmt::Type::I64,
            storage_ty: None,
        };
        let disc_col_id = self.create_column(
            disc_col_name.clone(),
            &disc_primitive,
            field.nullable,
            false,
        );

        let field_expr = field_expr(field, field_index, source_field_id, base_projection);

        // For data-carrying enums the model value is Record([I64(disc), ...]),
        // so project [0] to extract the discriminant; for unit-only enums the
        // value IS the I64 discriminant directly.
        let disc_expr = if embedded_enum.has_data_variants() {
            stmt::Expr::project(field_expr.clone(), stmt::Projection::single(0))
        } else {
            field_expr.clone()
        };

        let lowering = self.encode_column(disc_col_id, &stmt::Type::I64, disc_expr);
        let lowering_index = self.model_to_table.len();
        self.lowering_columns.push(disc_col_id);
        self.model_to_table.push(lowering);

        let bit = self.next_bit();

        let sub_projection = if source_field_id.is_some() {
            let mut proj = base_projection.clone();
            proj.push(field_index);
            proj
        } else {
            stmt::Projection::identity()
        };

        let disc_proj = stmt::Expr::project(field_expr.clone(), stmt::Projection::single(0));
        let mut variants = Vec::new();

        for variant in &embedded_enum.variants {
            let mut vf_fields = Vec::new();

            for (local_idx, vf) in variant.fields.iter().enumerate() {
                // Variant field columns are always nullable because only the
                // owning variant populates them.
                let vf_col_name = format_column_name(vf, None, Some(&disc_col_name));

                let vf_field = match &vf.ty {
                    app::FieldTy::Primitive(vf_primitive) => {
                        let vf_col_id =
                            self.create_column(vf_col_name, vf_primitive, true, false);

                        let arm = stmt::MatchArm {
                            pattern: stmt::Value::I64(variant.discriminant),
                            expr: stmt::Expr::project(
                                field_expr.clone(),
                                stmt::Projection::single(local_idx + 1),
                            ),
                        };

                        let vf_lowering_expr = self.encode_column(
                            vf_col_id,
                            &vf_primitive.ty,
                            stmt::Expr::match_expr(
                                disc_proj.clone(),
                                vec![arm],
                                stmt::Expr::null(),
                            ),
                        );
                        let vf_lowering = self.model_to_table.len();
                        self.lowering_columns.push(vf_col_id);
                        self.model_to_table.push(vf_lowering_expr);

                        mapping::Field::Primitive(mapping::FieldPrimitive {
                            column: vf_col_id,
                            lowering: vf_lowering,
                            field_mask: stmt::PathFieldSet::from_iter([bit]),
                            sub_projection: stmt::Projection::single(local_idx + 1),
                        })
                    }
                    app::FieldTy::Embedded(embedded) => {
                        let embedded_model = self.app.model(embedded.target);
                        let embedded_struct = embedded_model.expect_embedded_struct();
                        self.map_variant_struct_field(
                            local_idx,
                            &vf_col_name,
                            embedded_struct,
                            &field_expr,
                            &disc_proj,
                            variant.discriminant,
                            bit,
                        )
                    }
                    _ => panic!("unexpected field type in enum variant"),
                };

                vf_fields.push(vf_field);
            }

            variants.push(mapping::EnumVariant {
                discriminant: variant.discriminant,
                fields: vf_fields,
            });
        }

        mapping::Field::Enum(mapping::FieldEnum {
            disc_column: disc_col_id,
            disc_lowering: lowering_index,
            variants,
            field_mask: stmt::PathFieldSet::from_iter([bit]),
            sub_projection,
        })
    }

    fn map_field_struct(
        &mut self,
        field_index: usize,
        field: &app::Field,
        embedded_struct: &app::EmbeddedStruct,
        prefix: Option<&str>,
        source_field_id: Option<FieldId>,
        base_projection: &stmt::Projection,
    ) -> mapping::Field {
        let nested_prefix = format_column_name(field, None, prefix);
        let nested_source = source_field_id.or(Some(field.id));
        let nested_projection = if source_field_id.is_none() {
            stmt::Projection::identity()
        } else {
            let mut proj = base_projection.clone();
            proj.push(field_index);
            proj
        };

        let nested_fields = self.map_fields(
            &embedded_struct.fields,
            Some(&nested_prefix),
            nested_source,
            nested_projection.clone(),
        );

        let columns: indexmap::IndexMap<ColumnId, usize> =
            nested_fields.iter().flat_map(|f| f.columns()).collect();
        let field_mask = nested_fields
            .iter()
            .fold(stmt::PathFieldSet::new(), |acc, f| acc | f.field_mask());

        mapping::Field::Struct(mapping::FieldStruct {
            fields: nested_fields,
            columns,
            field_mask,
            sub_projection: nested_projection,
        })
    }

    /// Expands a struct-typed field inside a data-carrying enum variant into
    /// its flattened column representation.
    ///
    /// Each primitive sub-field of the struct becomes a standalone nullable
    /// column named `{vf_col_name}_{sub_field_name}`. The model_to_table
    /// lowering for each sub-field is wrapped in a discriminant match arm so
    /// only the owning variant populates the column.
    fn map_variant_struct_field(
        &mut self,
        local_idx: usize,
        vf_col_name: &str,
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

            let sub_col_name = format_column_name(sub_field, None, Some(vf_col_name));
            let sub_col_id = self.create_column(sub_col_name, sub_primitive, true, false);

            // The enum value is Record([I64(disc), vf_0, vf_1, ...]).
            // The struct field at local_idx is at position local_idx + 1.
            // The sub-field at sub_idx is at position sub_idx within the struct record.
            let mut arm_proj = stmt::Projection::single(local_idx + 1);
            arm_proj.push(sub_idx);

            let arm = stmt::MatchArm {
                pattern: stmt::Value::I64(discriminant),
                expr: stmt::Expr::project(field_expr.clone(), arm_proj.clone()),
            };

            let sub_lowering_expr = self.encode_column(
                sub_col_id,
                &sub_primitive.ty,
                stmt::Expr::match_expr(disc_proj.clone(), vec![arm], stmt::Expr::null()),
            );
            let sub_lowering = self.model_to_table.len();
            self.lowering_columns.push(sub_col_id);
            self.model_to_table.push(sub_lowering_expr);

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
fn field_expr(
    field: &app::Field,
    field_index: usize,
    source_field_id: Option<FieldId>,
    base_projection: &stmt::Projection,
) -> stmt::Expr {
    if let Some(source) = source_field_id {
        let base = stmt::Expr::ref_self_field(source);
        let mut projection = base_projection.clone();
        projection.push(field_index);
        stmt::Expr::project(base, projection)
    } else {
        stmt::Expr::ref_self_field(field.id)
    }
}

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

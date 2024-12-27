use super::*;

use std::iter::repeat;

impl Table {
    pub(crate) fn lower_models(&mut self, models: &mut [&mut Model]) {
        LowerModels {
            table: self,
            prefix_table_names: models.len() > 1,
        }
        .lower_models(models)
    }
}

/// Lower a set of models to a table
struct LowerModels<'a> {
    /// Which table to update with the lowered models
    table: &'a mut Table,

    /// When true, column names should be prefixed with their associated models
    prefix_table_names: bool,
}

/// Computes a model's lowering
struct ModelLoweringBuilder<'a> {
    table: &'a mut Table,
    lowering_columns: Vec<ColumnId>,
    model_to_table: Vec<stmt::Expr>,
    model_pk_to_table: Vec<stmt::Expr>,
    table_to_model: Vec<stmt::Expr>,
}

impl<'a> LowerModels<'a> {
    fn lower_models(&mut self, models: &mut [&mut Model]) {
        assert!(!models.is_empty());

        // Stub out the table primary key with enough fields to cover all models
        // being lowered. The table primary key will be populated with specifics
        // later.
        self.build_placeholder_primary_key(models);

        // Lower each model's primary key to the table.
        for model in models.iter_mut() {
            self.lower_model_primary_key(model);
        }

        // Sanity checks
        debug_assert_eq!(
            self.table.primary_key.columns.len(),
            self.table.indices[0].columns.len()
        );

        // Populate the rest of the columns
        for model in models.iter_mut() {
            self.lower_model_fields(model);
        }

        // Hax
        for column in &mut self.table.columns {
            if let stmt::Type::Enum(_) = column.ty {
                column.ty = stmt::Type::String;
            }
        }

        self.update_index_names();
    }

    fn build_placeholder_primary_key(&mut self, models: &mut [&mut Model]) {
        let num_pk_fields = models
            .iter()
            .map(|model| model.primary_key_primitives().count())
            .max()
            .unwrap();

        for i in 0..num_pk_fields {
            let column_id = ColumnId {
                table: self.table.id,
                index: i,
            };

            let mut scope = None;

            for model in &*models {
                let pk_index = &model.indices[0];
                assert!(pk_index.primary_key);

                let Some(pk_field) = pk_index.fields.get(i) else {
                    continue;
                };

                match scope {
                    None => scope = Some(pk_field.scope),
                    Some(scope) => {
                        assert_eq!(scope, pk_field.scope);
                    }
                }
            }

            self.table.primary_key.columns.push(column_id);
            self.table.indices[0].columns.push(IndexColumn {
                column: column_id,
                // TODO: we don't actually know what the columns will be yet...
                op: IndexOp::Eq,
                scope: scope.unwrap(),
            });
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

                index.name.push_str("_");
                index.name.push_str(&column.name);
            }
        }
    }

    fn lower_model_primary_key(&mut self, model: &mut Model) {
        let tys = model
            .primary_key_primitives()
            .map(|primitive| Some(primitive.ty.clone()))
            .chain(repeat(None))
            .take(self.table.primary_key.columns.len());

        // Compute the column type
        for (i, ty) in tys.enumerate() {
            let column_id = ColumnId {
                table: self.table.id,
                index: i,
            };

            if let Some(column) = self.table.columns.get_mut(i) {
                column.name = format!("key_{i}");

                match &mut column.ty {
                    stmt::Type::Enum(ty_enum) => {
                        for variant in &ty_enum.variants {
                            match &variant.fields[..] {
                                [] => {
                                    assert!(ty.is_some());
                                }
                                [variant_ty] => {
                                    assert!(ty.is_none() || ty.as_ref().unwrap() == variant_ty);
                                }
                                _ => todo!(),
                            }
                        }

                        let variant = ty_enum.insert_variant();

                        if let Some(ty) = ty {
                            variant.fields.push(ty.clone());
                        }
                    }
                    _ => {
                        if let Some(ty) = ty {
                            if column.ty != ty {
                                let mut ty_enum = stmt::TypeEnum::default();
                                // Insert a variant for the current previous column type
                                ty_enum.insert_variant().fields.push(column.ty.clone());

                                // Insert a variant for the new type
                                ty_enum.insert_variant().fields.push(ty);

                                column.ty = ty_enum.into();
                            }
                        } else {
                            assert_eq!(column.ty, stmt::Type::Null);

                            // Go straight to an enum
                            let mut ty_enum = stmt::TypeEnum::default();
                            ty_enum.insert_variant();
                            column.ty = ty_enum.into();
                        }
                    }
                }
            } else {
                // Get the column name
                // TODO: this probably isn't right...
                let name = model
                    .primary_key
                    .fields
                    .get(i)
                    .map(|field_id| model.field(field_id).name.clone())
                    .unwrap_or_else(|| format!("key_{i}"));

                // If unit type, go straight to enum
                //
                // TODO: null probably isn't the right type... maybe `ty` should
                // be Option<Type> if we don't know what it is yet.
                let ty = match ty {
                    Some(ty) => stmt_ty_to_table(ty),
                    None => {
                        let mut ty_enum = stmt::TypeEnum::default();
                        ty_enum.insert_variant();
                        ty_enum.into()
                    }
                };

                assert_eq!(self.table.columns.len(), i);
                self.table.columns.push(Column {
                    id: column_id,
                    name,
                    ty,
                    nullable: false,
                    primary_key: true,
                });
            }
        }

        for (i, primitive) in model.primary_key_primitives_mut().enumerate() {
            primitive.column = ColumnId {
                table: self.table.id,
                index: i,
            };
        }
    }

    fn lower_model_fields(&mut self, model: &mut Model) {
        let prefix = if self.prefix_table_names {
            Some(name_from_model(&model.name))
        } else {
            None
        };

        // First, populate columns
        for field in &mut model.fields {
            // Skip PK fields
            if field.primary_key {
                continue;
            }

            match &mut field.ty {
                FieldTy::Primitive(simple) => {
                    self.create_column_for_primitive(
                        simple,
                        &field.name,
                        prefix.as_deref(),
                        field.nullable,
                    );
                }
                // HasMany/HasOne relationships do not have columns... for now?
                FieldTy::BelongsTo(_) | FieldTy::HasMany(_) | FieldTy::HasOne(_) => {}
            }
        }

        ModelLoweringBuilder {
            table: self.table,
            lowering_columns: vec![],
            model_to_table: vec![],
            model_pk_to_table: vec![],
            table_to_model: vec![],
        }
        .build_lowering(model);

        self.populate_model_indices(model);
    }

    fn populate_model_indices(&mut self, model: &mut Model) {
        for model_index in &mut model.indices {
            // Skip the primary key index
            if model_index.primary_key {
                model_index.lowering.index = self.table.indices[0].id;
                continue;
            }

            let mut index = Index {
                id: IndexId {
                    table: self.table.id,
                    index: self.table.indices.len(),
                },
                name: String::new(),
                on: self.table.id,
                columns: vec![],
                unique: model_index.unique,
                primary_key: false,
            };

            for index_field in &model_index.fields {
                match &model.fields[index_field.field.index].ty {
                    FieldTy::Primitive(simple) => index.columns.push(IndexColumn {
                        column: simple.column,
                        op: index_field.op,
                        scope: index_field.scope,
                    }),
                    FieldTy::BelongsTo(_) => todo!(),
                    FieldTy::HasMany(_) => todo!(),
                    FieldTy::HasOne(_) => todo!(),
                }
            }

            // Define lowering
            model_index.lowering.index = index.id;

            self.table.indices.push(index);
        }
    }

    fn create_column_for_primitive(
        &mut self,
        primitive: &mut FieldPrimitive,
        name: &str,
        prefix: Option<&str>,
        nullable: bool,
    ) {
        let name = if let Some(prefix) = prefix {
            format!("{prefix}__{name}")
        } else {
            name.to_string()
        };

        let column = Column {
            id: ColumnId {
                table: self.table.id,
                index: self.table.columns.len(),
            },
            name,
            ty: stmt_ty_to_table(primitive.ty.clone()),
            nullable,
            primary_key: false,
        };

        primitive.column = column.id;

        self.table.columns.push(column);
    }
}

impl<'a> ModelLoweringBuilder<'a> {
    fn build_lowering(mut self, model: &mut Model) {
        self.map_model_fields_to_columns(model);

        // The primary key columns are always featured in model lowerings,
        // even if the model does not have an equivalent (in which case, we
        // generate a placeholder value).
        for pk in &self.table.primary_key.columns {
            if !self.lowering_columns.contains(pk) {
                let ty_enum = match &self.table.column(pk).ty {
                    stmt::Type::Enum(ty_enum) => ty_enum,
                    _ => todo!(),
                };

                let variant = ty_enum
                    .variants
                    .iter()
                    .find(|variant| variant.fields.is_empty())
                    .unwrap();

                self.lowering_columns.push(*pk);
                // TODO: this should not be hard coded
                self.model_to_table
                    .push(format!("{}#", variant.discriminant).into());
            }
        }

        assert!(self.model_to_table.len() > 0);
        assert_eq!(self.model_to_table.len(), self.lowering_columns.len());

        // Iterate fields again (including PK fields) and build the table -> model map.
        for field in &model.fields {
            match &field.ty {
                FieldTy::Primitive(primitive) => {
                    let expr = self.map_table_column_to_model(primitive);
                    self.table_to_model.push(expr);
                }
                FieldTy::BelongsTo(_) | FieldTy::HasMany(_) | FieldTy::HasOne(_) => {
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

                for (i, field_id) in model.primary_key.fields.iter().enumerate() {
                    if field_id.index == *step {
                        let mut p = projection.clone();
                        p[0] = i.into();

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

        model.lowering.columns = self.lowering_columns;
        model.lowering.model_to_table = stmt::ExprRecord::from_vec(self.model_to_table);
        model.lowering.table_to_model = stmt::ExprRecord::from_vec(self.table_to_model);
        model.lowering.model_pk_to_table = if self.model_pk_to_table.len() == 1 {
            let expr = self.model_pk_to_table.into_iter().next().unwrap();
            debug_assert!(expr.is_field() || expr.is_cast(), "expr={:#?}", expr);
            expr
        } else {
            stmt::ExprRecord::from_vec(self.model_pk_to_table).into()
        };
    }

    fn map_model_fields_to_columns(&mut self, model: &mut Model) {
        for field in &mut model.fields {
            match &mut field.ty {
                FieldTy::Primitive(primitive) => {
                    assert_ne!(primitive.column, ColumnId::placeholder());
                    self.map_primitive(field.id, primitive);
                }
                FieldTy::BelongsTo(_) | FieldTy::HasMany(_) | FieldTy::HasOne(_) => {}
            }
        }
    }

    fn map_primitive(&mut self, expr: impl Into<stmt::Expr>, primitive: &mut FieldPrimitive) {
        let lowering = self.encode_column(primitive.column, &primitive.ty, expr);
        primitive.lowering = self.model_to_table.len();

        self.lowering_columns.push(primitive.column);
        self.model_to_table.push(lowering);
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
            stmt::Type::Enum(ty_enum) => {
                let variant = ty_enum
                    .variants
                    .iter()
                    .find(|variant| match &variant.fields[..] {
                        [field_ty] if field_ty == ty => true,
                        _ => false,
                    })
                    .unwrap();

                stmt::Expr::concat_str((
                    variant.discriminant.to_string(),
                    "#",
                    stmt::Expr::cast(expr, stmt::Type::String),
                ))
            }
            stmt::Type::String if ty.is_id() => stmt::Expr::cast(expr, &column.ty),
            _ => todo!("column={column:#?}"),
        }
    }

    fn map_table_column_to_model(&mut self, primitive: &FieldPrimitive) -> stmt::Expr {
        let column_id = primitive.column;
        let column = self.table.column(column_id);

        match &column.ty {
            c_ty if *c_ty == primitive.ty => stmt::Expr::column(column),
            stmt::Type::Enum(ty_enum) => {
                let variant = ty_enum
                    .variants
                    .iter()
                    .find(|variant| match &variant.fields[..] {
                        [field_ty] => *field_ty == primitive.ty,
                        _ => false,
                    })
                    .unwrap();

                stmt::Expr::DecodeEnum(
                    Box::new(stmt::Expr::column(column)),
                    primitive.ty.clone(),
                    variant.discriminant,
                )
            }
            stmt::Type::String if primitive.ty.is_id() => {
                stmt::Expr::cast(stmt::Expr::column(column), &primitive.ty)
            }
            _ => todo!("column={column:#?}; primitive={primitive:#?}"),
        }
    }
}

fn stmt_ty_to_table(ty: stmt::Type) -> stmt::Type {
    match ty {
        stmt::Type::I64 => stmt::Type::I64,
        stmt::Type::String => stmt::Type::String,
        stmt::Type::Id(_) => stmt::Type::String,
        _ => todo!("{ty:#?}"),
    }
}

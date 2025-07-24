use super::*;

struct BuildTableFromModels<'a> {
    /// Database-specific capabilities
    _db: &'a driver::Capability,

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
    table: &'a mut Table,
    mapping: &'a mut mapping::Model,
    lowering_columns: Vec<ColumnId>,
    model_to_table: Vec<stmt::Expr>,
    model_pk_to_table: Vec<stmt::Expr>,
    table_to_model: Vec<stmt::Expr>,
}

impl BuildSchema<'_> {
    pub(super) fn build_table_stub_for_model(&mut self, model: &Model) -> TableId {
        if let Some(table_name) = &model.table_name {
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
                .filter(|model| self.mapping.model(model.id).table == table.id)
                .collect::<Vec<_>>();

            BuildTableFromModels {
                _db: db,
                table,
                mapping: &mut self.mapping,
                prefix_table_names: models.len() > 1,
            }
            .build(&models);
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
    fn build(&mut self, models: &[&Model]) {
        assert!(!models.is_empty());

        // Stub out the table primary key with enough fields to cover all models
        // being lowered. The table primary key will be populated with specifics
        // later.
        self.build_placeholder_primary_key(models);

        // Lower each model's primary key to the table.
        for model in models {
            self.map_model_primary_key(model);
        }

        // Sanity checks
        debug_assert_eq!(
            self.table.primary_key.columns.len(),
            self.table.indices[0].columns.len()
        );

        // Populate the rest of the columns
        for model in models {
            self.map_model_fields(model);
        }

        // Hax
        for column in &mut self.table.columns {
            if let stmt::Type::Enum(_) = column.ty {
                column.ty = stmt::Type::String;
            }
        }

        self.update_index_names();
    }

    fn build_placeholder_primary_key(&mut self, models: &[&Model]) {
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

            for model in models {
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
            self.table.indices[0].columns.push(db::IndexColumn {
                column: column_id,
                // TODO: we don't actually know what the columns will be yet...
                op: db::IndexOp::Eq,
                scope: scope.unwrap(),
            });
        }
    }

    fn map_model_primary_key(&mut self, model: &Model) {
        let tys = model
            .primary_key_primitives()
            .map(|primitive| Some(primitive.ty.clone()))
            .chain(std::iter::repeat(None))
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
                    .map(|field_id| model.field(*field_id).name.clone())
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
                self.table.columns.push(db::Column {
                    id: column_id,
                    name,
                    ty,
                    storage_ty: None,
                    nullable: false,
                    primary_key: true,
                });
            }
        }

        let mapping = self.mapping.model_mut(model);

        for (i, field) in model.primary_key_fields().enumerate() {
            let Some(mapping) = &mut mapping.fields[field.id.index] else {
                todo!()
            };
            mapping.column = ColumnId {
                table: self.table.id,
                index: i,
            };
        }
    }

    fn map_model_fields(&mut self, model: &Model) {
        let prefix = if self.prefix_table_names {
            Some(model.name.snake_case())
        } else {
            None
        };

        // First, populate columns
        for field in &model.fields {
            // Skip PK fields
            if field.primary_key {
                continue;
            }

            match &field.ty {
                app::FieldTy::Primitive(simple) => {
                    self.create_column_for_primitive(
                        field.id,
                        simple,
                        &field.name,
                        prefix.as_deref(),
                        field.nullable,
                    );
                }
                // HasMany/HasOne relationships do not have columns... for now?
                app::FieldTy::BelongsTo(_) | app::FieldTy::HasMany(_) | app::FieldTy::HasOne(_) => {
                }
            }
        }

        BuildMapping {
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
            // Skip the primary key index
            if model_index.primary_key {
                continue;
            }

            let mut index = db::Index {
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
                let column = self.mapping.model(model.id).fields[index_field.field.index]
                    .as_ref()
                    .unwrap()
                    .column;

                match &model.fields[index_field.field.index].ty {
                    app::FieldTy::Primitive(_) => index.columns.push(db::IndexColumn {
                        column,
                        op: index_field.op,
                        scope: index_field.scope,
                    }),
                    app::FieldTy::BelongsTo(_) => todo!(),
                    app::FieldTy::HasMany(_) => todo!(),
                    app::FieldTy::HasOne(_) => todo!(),
                }
            }

            self.table.indices.push(index);
        }
    }

    fn create_column_for_primitive(
        &mut self,
        field_id: FieldId,
        primitive: &app::FieldPrimitive,
        name: &str,
        prefix: Option<&str>,
        nullable: bool,
    ) {
        let name = if let Some(prefix) = prefix {
            format!("{prefix}__{name}")
        } else {
            name.to_string()
        };

        let column = db::Column {
            id: ColumnId {
                table: self.table.id,
                index: self.table.columns.len(),
            },
            name,
            ty: stmt_ty_to_table(primitive.ty.clone()),
            storage_ty: primitive.storage_ty.clone(),
            nullable,
            primary_key: false,
        };

        self.mapping.model_mut(field_id.model).fields[field_id.index]
            .as_mut()
            .unwrap()
            .column = column.id;

        self.table.columns.push(column);
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
        self.map_model_fields_to_columns(model);

        // The primary key columns are always featured in model lowerings,
        // even if the model does not have an equivalent (in which case, we
        // generate a placeholder value).
        for pk in &self.table.primary_key.columns {
            if !self.lowering_columns.contains(pk) {
                let ty_enum = match &self.table.column(*pk).ty {
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

        assert!(!self.model_to_table.is_empty());
        assert_eq!(self.model_to_table.len(), self.lowering_columns.len());

        // Iterate fields again (including PK fields) and build the table -> model map.
        for field in &model.fields {
            match &field.ty {
                app::FieldTy::Primitive(primitive) => {
                    let expr = self.map_table_column_to_model(field.id, primitive);
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

        self.mapping.columns = self.lowering_columns;
        self.mapping.model_to_table = stmt::ExprRecord::from_vec(self.model_to_table);
        self.mapping.table_to_model = stmt::ExprRecord::from_vec(self.table_to_model);
        self.mapping.model_pk_to_table = if self.model_pk_to_table.len() == 1 {
            let expr = self.model_pk_to_table.into_iter().next().unwrap();
            debug_assert!(expr.is_field() || expr.is_cast(), "expr={expr:#?}");
            expr
        } else {
            stmt::ExprRecord::from_vec(self.model_pk_to_table).into()
        };
    }

    fn map_model_fields_to_columns(&mut self, model: &Model) {
        for field in &model.fields {
            match &field.ty {
                app::FieldTy::Primitive(primitive) => {
                    let mapping = self.mapping.fields[field.id.index].as_ref().unwrap();
                    assert_ne!(mapping.column, ColumnId::placeholder());
                    self.map_primitive(field.id, primitive);
                }
                app::FieldTy::BelongsTo(_) | app::FieldTy::HasMany(_) | app::FieldTy::HasOne(_) => {
                }
            }
        }
    }

    fn map_primitive(&mut self, field: FieldId, primitive: &app::FieldPrimitive) {
        let column = self.mapping.fields[field.index].as_ref().unwrap().column;
        let lowering = self.encode_column(column, &primitive.ty, field);

        self.mapping.fields[field.index].as_mut().unwrap().lowering = self.model_to_table.len();

        self.lowering_columns.push(column);
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
                    .find(|variant| matches!(&variant.fields[..], [field_ty] if field_ty == ty))
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

    fn map_table_column_to_model(
        &mut self,
        field_id: FieldId,
        primitive: &app::FieldPrimitive,
    ) -> stmt::Expr {
        let column_id = self.mapping.fields[field_id.index].as_ref().unwrap().column;
        let column = self.table.column(column_id);

        match &column.ty {
            c_ty if *c_ty == primitive.ty => stmt::Expr::column(column.id),
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
                    Box::new(stmt::Expr::column(column.id)),
                    primitive.ty.clone(),
                    variant.discriminant,
                )
            }
            stmt::Type::String if primitive.ty.is_id() => {
                stmt::Expr::cast(stmt::Expr::column(column.id), &primitive.ty)
            }
            _ => todo!("column={column:#?}; primitive={primitive:#?}"),
        }
    }
}

fn stmt_ty_to_table(ty: stmt::Type) -> stmt::Type {
    match ty {
        stmt::Type::I32 => stmt::Type::I32,
        stmt::Type::I64 => stmt::Type::I64,
        stmt::Type::String => stmt::Type::String,
        stmt::Type::Id(_) => stmt::Type::String,
        _ => todo!("{ty:#?}"),
    }
}

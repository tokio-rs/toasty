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

            assert!(
                models.len() == 1,
                "TODO: handle mapping many models to one table"
            );

            BuildTableFromModels {
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
        let prefix = if self.prefix_table_names {
            Some(model.name.snake_case())
        } else {
            None
        };

        // First, populate columns
        for field in &model.fields {
            match &field.ty {
                app::FieldTy::Primitive(simple) => {
                    self.create_column_for_primitive(field, simple, prefix.as_deref());
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

                if model_index.primary_key {
                    self.table.primary_key.columns.push(column);
                }
            }

            self.table.indices.push(index);
        }
    }

    fn create_column_for_primitive(
        &mut self,
        field: &app::Field,
        primitive: &app::FieldPrimitive,
        prefix: Option<&str>,
    ) {
        let storage_name = if let Some(prefix) = prefix {
            let storage_name = field.name.storage_name();
            format!("{prefix}__{storage_name}")
        } else {
            field.name.storage_name().to_owned()
        };

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
            name: storage_name,
            ty: storage_ty.bridge_type(&primitive.ty),
            storage_ty,
            nullable: field.nullable,
            primary_key: false,
            auto_increment: auto_increment && self.db.auto_increment,
        };

        self.mapping.model_mut(field.id.model).fields[field.id.index]
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
        let lowering = self.encode_column(column, &primitive.ty, stmt::Expr::ref_self_field(field));

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
        field_id: FieldId,
        primitive: &app::FieldPrimitive,
    ) -> stmt::Expr {
        let column_id = self.mapping.fields[field_id.index].as_ref().unwrap().column;
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
}

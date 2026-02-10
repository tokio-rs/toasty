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
                app::FieldTy::Embedded(embedded) => {
                    let field_prefix = field.name.storage_name().to_owned();
                    self.flatten_embedded_fields(embedded.target, &field_prefix);
                }
                // HasMany/HasOne relationships do not have columns... for now?
                app::FieldTy::BelongsTo(_) | app::FieldTy::HasMany(_) | app::FieldTy::HasOne(_) => {
                }
            }
        }

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
            .as_primitive_mut()
            .unwrap()
            .column = column.id;

        self.table.columns.push(column);
    }

    fn create_column_for_embedded_primitive(
        &mut self,
        column_name: &str,
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
            name: column_name.to_owned(),
            ty: storage_ty.bridge_type(&primitive.ty),
            storage_ty,
            nullable: field.nullable,
            primary_key: false,
            auto_increment: auto_increment && self.db.auto_increment,
        };

        // Note: We do NOT update field mapping here. Embedded fields don't have
        // direct mappings - the parent field will be mapped to multiple columns.
        // This will be handled in a future phase.

        self.table.columns.push(column);
    }

    /// Recursively flattens embedded struct fields into database columns.
    ///
    /// For each field in the embedded model:
    /// - Primitive fields become columns with names like `{prefix}_{field_name}`
    /// - Nested embedded fields are recursively flattened with accumulated prefixes
    /// - Relations are not allowed and will panic
    fn flatten_embedded_fields(&mut self, embedded_model_id: app::ModelId, prefix: &str) {
        let embedded_model = self.app.model(embedded_model_id);

        for embedded_field in &embedded_model.fields {
            match &embedded_field.ty {
                app::FieldTy::Primitive(primitive) => {
                    let column_name = format!("{}_{}", prefix, embedded_field.name.storage_name());
                    self.create_column_for_embedded_primitive(
                        &column_name,
                        embedded_field,
                        primitive,
                    );
                }
                app::FieldTy::Embedded(_nested_embedded) => {
                    // TODO: Handle nested embedded structs by making this recursive
                    todo!("nested embedded structs not yet implemented")
                }
                // Relations are not allowed in embedded types
                app::FieldTy::BelongsTo(_) | app::FieldTy::HasMany(_) | app::FieldTy::HasOne(_) => {
                    panic!("relations not allowed in embedded types")
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
        self.map_model_fields_to_columns(model);

        assert!(!self.model_to_table.is_empty());
        assert_eq!(self.model_to_table.len(), self.lowering_columns.len());

        // Iterate fields again (including PK fields) and build the table -> model map.
        for (field_index, field) in model.fields.iter().enumerate() {
            match &field.ty {
                app::FieldTy::Primitive(primitive) => {
                    let expr = self.map_table_column_to_model(field.id, primitive);
                    self.table_to_model.push(expr);
                }
                app::FieldTy::Embedded(_embedded) => {
                    // Use the mapping information already built during map_embedded
                    let field_mapping = &self.mapping.fields[field_index];
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
                    let mapping = &self.mapping.fields[field.id.index];
                    assert_ne!(
                        mapping.as_primitive().unwrap().column,
                        ColumnId::placeholder()
                    );
                    self.map_primitive(field.id, primitive);
                }
                app::FieldTy::Embedded(embedded) => {
                    let field_prefix = field.name.storage_name();
                    self.map_embedded(model, field.id, embedded.target, field_prefix);
                }
                app::FieldTy::BelongsTo(_) | app::FieldTy::HasMany(_) | app::FieldTy::HasOne(_) => {
                }
            }
        }
    }

    fn map_primitive(&mut self, field: FieldId, primitive: &app::FieldPrimitive) {
        let column = self.mapping.fields[field.index]
            .as_primitive()
            .unwrap()
            .column;
        let lowering = self.encode_column(column, &primitive.ty, stmt::Expr::ref_self_field(field));

        self.mapping.fields[field.index]
            .as_primitive_mut()
            .unwrap()
            .lowering = self.model_to_table.len();

        self.lowering_columns.push(column);
        self.model_to_table.push(lowering);
    }

    fn map_embedded(
        &mut self,
        _source_model: &Model,
        source_field_id: FieldId,
        target_model_id: app::ModelId,
        prefix: &str,
    ) {
        let target_model = self.app.model(target_model_id);
        let mut embedded_fields = Vec::new();

        for (target_field_index, target_field) in target_model.fields.iter().enumerate() {
            match &target_field.ty {
                app::FieldTy::Primitive(primitive) => {
                    // Find the column by name pattern: {prefix}_{field_name}
                    let column_name = format!("{}_{}", prefix, target_field.name.storage_name());
                    let column_id = self
                        .table
                        .columns
                        .iter()
                        .find(|col| col.name == column_name)
                        .map(|col| col.id)
                        .expect("column should exist for embedded primitive field");

                    // Create a projection expression that accesses the target field:
                    // Project the source field by the target field's index
                    // e.g., user.address[0] for street, user.address[1] for city
                    let base = stmt::Expr::ref_self_field(source_field_id);
                    let projection = stmt::Projection::from([target_field_index]);
                    let expr = stmt::Expr::project(base, projection);
                    let lowering = self.encode_column(column_id, &primitive.ty, expr);

                    // Track this field in the embedded field mapping
                    embedded_fields.push(mapping::Field::Primitive(mapping::FieldPrimitive {
                        column: column_id,
                        lowering: self.model_to_table.len(),
                    }));

                    self.lowering_columns.push(column_id);
                    self.model_to_table.push(lowering);
                }
                app::FieldTy::Embedded(_) => {
                    // TODO: Handle nested embedded structs recursively
                    embedded_fields.push(mapping::Field::Relation);
                    todo!("nested embedded structs not yet implemented")
                }
                app::FieldTy::BelongsTo(_) | app::FieldTy::HasMany(_) | app::FieldTy::HasOne(_) => {
                    // Relations not allowed - push Relation
                    embedded_fields.push(mapping::Field::Relation);
                    panic!("relations not allowed in embedded types")
                }
            }
        }

        // Update the embedded field mapping with the collected field mappings
        if let mapping::Field::Embedded(field_embedded) =
            &mut self.mapping.fields[source_field_id.index]
        {
            field_embedded.fields = embedded_fields;
        }
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
        let column_id = self.mapping.fields[field_id.index]
            .as_primitive()
            .unwrap()
            .column;
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
                mapping::Field::Embedded(_) => {
                    // TODO: Handle nested embedded structs recursively
                    todo!("nested embedded structs not yet implemented")
                }
                mapping::Field::Relation => {
                    panic!("relations not allowed in embedded types")
                }
            }
        }

        // Build a record expression from the field expressions
        stmt::Expr::record(field_exprs)
    }
}

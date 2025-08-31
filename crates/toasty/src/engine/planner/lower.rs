use super::{simplify, Planner};
use crate::engine::typed::Typed;
use toasty_core::{
    schema::{
        app::{self, FieldId},
        db::{Column, Table},
        mapping, Schema,
    },
    stmt::{self, VisitMut},
};

struct LowerStatement<'a> {
    schema: &'a Schema,

    /// The model in which the statement is contextualized.
    model: &'a app::Model,

    /// The associated table for the model.
    table: &'a Table,

    /// How to map expressions between the model and table
    mapping: &'a mapping::Model,
}

/// Substitute fields for columns
struct Substitute<I>(I);

trait Input {
    fn resolve_field(&mut self, field_id: FieldId) -> stmt::Expr;
}

impl<'a> LowerStatement<'a> {
    fn from_model(schema: &'a Schema, model: &'a app::Model) -> Self {
        LowerStatement {
            schema,
            model,
            table: schema.table_for(model),
            mapping: schema.mapping_for(model),
        }
    }
}

impl Planner<'_> {
    pub(crate) fn lower_stmt_delete(
        &self,
        model: &app::Model,
        typed_stmt: &mut Typed<stmt::Delete>,
    ) {
        LowerStatement::from_model(self.schema, model).visit_stmt_delete_mut(&mut typed_stmt.value);
        simplify::simplify_stmt(self.schema, &mut typed_stmt.value);
    }

    pub(crate) fn lower_stmt_query(&self, model: &app::Model, typed_stmt: &mut Typed<stmt::Query>) {
        // Extract includes before visitor runs - collect into Vec to avoid borrow issues
        let includes: Vec<stmt::Path> = match &typed_stmt.value.body {
            stmt::ExprSet::Select(select) => match &select.source {
                stmt::Source::Model(source) => source.include.clone(),
                _ => Vec::new(),
            },
            _ => Vec::new(),
        };

        // Convert the type from model-level to table-level types
        let lowered_type = self.build_lowered_type(model, &includes);
        typed_stmt.ty = lowered_type;

        // If we have Returning::Star and includes, build custom table_to_model
        if !includes.is_empty() {
            if let stmt::ExprSet::Select(select) = &mut typed_stmt.value.body {
                if matches!(select.returning, stmt::Returning::Star) {
                    let custom_table_to_model =
                        self.build_include_aware_table_to_model(model, &includes);
                    select.returning = stmt::Returning::Expr(custom_table_to_model.into());
                }
            }
        }

        // Now run the normal visitor (which will handle non-Star returning)
        LowerStatement::from_model(self.schema, model).visit_stmt_query_mut(&mut typed_stmt.value);
        simplify::simplify_stmt(self.schema, &mut typed_stmt.value);
    }

    pub(crate) fn lower_stmt_insert(
        &self,
        model: &app::Model,
        typed_stmt: &mut Typed<stmt::Insert>,
    ) {
        LowerStatement::from_model(self.schema, model).visit_stmt_insert_mut(&mut typed_stmt.value);
        simplify::simplify_stmt(self.schema, &mut typed_stmt.value);
    }

    pub(crate) fn lower_stmt_update(
        &self,
        model: &app::Model,
        typed_stmt: &mut Typed<stmt::Update>,
    ) {
        LowerStatement::from_model(self.schema, model).visit_stmt_update_mut(&mut typed_stmt.value);
        simplify::simplify_stmt(self.schema, &mut typed_stmt.value);
    }

    /// Build the lowered type with only primitive types (no model-level types)
    fn build_lowered_type(&self, model: &app::Model, includes: &[stmt::Path]) -> stmt::Type {
        let model_mapping = self.schema.mapping.model(model.id);
        let record_ty = model_mapping.record_ty.clone();

        // Convert model-level types to primitive types based on includes
        let lowered_record = self.resolve_to_primitive_types(&record_ty, includes, model);

        // For queries, wrap the record type in a List
        stmt::Type::List(Box::new(lowered_record))
    }

    /// Recursively resolve all model-level types to primitive types
    /// The includes parameter controls which associations get resolved vs become Null
    fn resolve_to_primitive_types(
        &self,
        ty: &stmt::Type,
        includes: &[stmt::Path],
        model: &app::Model,
    ) -> stmt::Type {
        match ty {
            stmt::Type::Record(field_types) => {
                let resolved_fields = field_types
                    .iter()
                    .enumerate()
                    .map(|(field_idx, field_ty)| {
                        // Extract includes for this specific field
                        let field_includes: Vec<stmt::Path> = includes
                            .iter()
                            .filter_map(|path| {
                                match &path.projection[..] {
                                    [idx, rest @ ..] if *idx == field_idx => {
                                        // This field is included, return remaining path for nested includes
                                        Some(stmt::Path {
                                            root: path.root,
                                            projection: stmt::Projection::from(rest),
                                        })
                                    }
                                    _ => None,
                                }
                            })
                            .collect();

                        // Handle the field type based on whether it's included
                        match field_ty {
                            stmt::Type::Model(target_id) => {
                                if !field_includes.is_empty() {
                                    // This association is included - resolve to its record type
                                    let target_model = self.schema.app.model(*target_id);
                                    let target_mapping = self.schema.mapping.model(*target_id);

                                    // Recurse with the nested includes for this field
                                    self.resolve_to_primitive_types(
                                        &target_mapping.record_ty,
                                        &field_includes,
                                        target_model,
                                    )
                                } else {
                                    // Association not included, return Null
                                    stmt::Type::Null
                                }
                            }
                            stmt::Type::List(inner) if matches!(**inner, stmt::Type::Model(_)) => {
                                if !field_includes.is_empty() {
                                    // HasMany relationship is included
                                    let stmt::Type::Model(target_id) = **inner else {
                                        unreachable!()
                                    };

                                    let target_model = self.schema.app.model(target_id);
                                    let target_mapping = self.schema.mapping.model(target_id);

                                    let inner_type = self.resolve_to_primitive_types(
                                        &target_mapping.record_ty,
                                        &field_includes,
                                        target_model,
                                    );

                                    stmt::Type::List(Box::new(inner_type))
                                } else {
                                    // HasMany not included, return Null
                                    stmt::Type::Null
                                }
                            }
                            // Already primitive or already Null
                            stmt::Type::Null => stmt::Type::Null,
                            stmt::Type::Id(_) => stmt::Type::String,
                            stmt::Type::Key(_) => stmt::Type::String,
                            stmt::Type::ForeignKey(_) => stmt::Type::String,
                            _ => {
                                // Other types pass through unchanged (already primitive)
                                field_ty.clone()
                            }
                        }
                    })
                    .collect();

                stmt::Type::Record(resolved_fields)
            }
            // If we somehow get a non-Record at the top level, handle it
            stmt::Type::Model(model_id) => {
                // This shouldn't happen at the top level, but handle it anyway
                let target_mapping = self.schema.mapping.model(*model_id);
                self.resolve_to_primitive_types(&target_mapping.record_ty, includes, model)
            }
            stmt::Type::List(inner) => stmt::Type::List(Box::new(
                self.resolve_to_primitive_types(inner, includes, model),
            )),
            // Convert other model-level types to primitives
            stmt::Type::Id(_) => stmt::Type::String,
            stmt::Type::Key(_) => stmt::Type::String,
            stmt::Type::ForeignKey(_) => stmt::Type::String,
            // Primitive types and other lowered types pass through unchanged
            _ => ty.clone(),
        }
    }

    /// Build a custom table_to_model expression that matches the lowered type
    fn build_include_aware_table_to_model(
        &self,
        model: &app::Model,
        includes: &[stmt::Path],
    ) -> stmt::ExprRecord {
        let model_mapping = self.schema.mapping.model(model.id);
        let mut fields = vec![];

        for (i, field) in model.fields.iter().enumerate() {
            let is_included = includes
                .iter()
                .any(|inc| matches!(&inc.projection[..], [idx] if *idx == i));

            if is_included && field.ty.is_relation() {
                // For included relations, we'll populate with actual data later
                // For now, use the base mapping but mark for population
                fields.push(model_mapping.table_to_model[i].clone());
            } else {
                fields.push(model_mapping.table_to_model[i].clone());
            }
        }

        stmt::ExprRecord::from_vec(fields)
    }
}

fn is_eq_constrained(expr: &stmt::Expr, column: &Column) -> bool {
    use stmt::Expr::*;

    match expr {
        And(expr) => expr.iter().any(|expr| is_eq_constrained(expr, column)),
        Or(expr) => expr.iter().all(|expr| is_eq_constrained(expr, column)),
        BinaryOp(expr) => {
            if !expr.op.is_eq() {
                return false;
            }

            match (&*expr.lhs, &*expr.rhs) {
                (Column(lhs), _) => lhs.references(column.id),
                (_, Column(rhs)) => rhs.references(column.id),
                _ => false,
            }
        }
        InList(expr) => match &*expr.expr {
            Column(lhs) => lhs.references(column.id),
            _ => todo!("expr={:#?}", expr),
        },
        _ => todo!("expr={:#?}", expr),
    }
}

impl VisitMut for LowerStatement<'_> {
    fn visit_assignments_mut(&mut self, i: &mut stmt::Assignments) {
        let mut assignments = stmt::Assignments::default();

        for index in i.keys() {
            let field = &self.model.fields[index];

            if field.primary_key {
                todo!("updating PK not supported yet");
            }

            match &field.ty {
                app::FieldTy::Primitive(_) => {
                    let Some(field_mapping) = &self.mapping.fields[index] else {
                        todo!()
                    };

                    let mut lowered = self.mapping.model_to_table[field_mapping.lowering].clone();
                    Substitute(&*i).visit_expr_mut(&mut lowered);
                    assignments.set(field_mapping.column, lowered);
                }
                _ => {
                    todo!("field = {:#?};", field);
                }
            }
        }

        *i = assignments;
    }

    fn visit_expr_set_op_mut(&mut self, i: &mut stmt::ExprSetOp) {
        todo!("stmt={i:#?}");
    }

    fn visit_expr_mut(&mut self, i: &mut stmt::Expr) {
        stmt::visit_mut::visit_expr_mut(self, i);

        let maybe_expr = match i {
            stmt::Expr::BinaryOp(expr) => {
                self.lower_expr_binary_op(expr.op, &mut expr.lhs, &mut expr.rhs)
            }
            stmt::Expr::Reference(stmt::ExprReference::Field { model: _, index }) => {
                *i = self.mapping.table_to_model[*index].clone();

                self.visit_expr_mut(i);
                return;
            }
            stmt::Expr::InList(expr) => self.lower_expr_in_list(&mut expr.expr, &mut expr.list),
            stmt::Expr::InSubquery(expr) => {
                let sub_model = self
                    .schema
                    .app
                    .model(expr.query.body.as_select().source.as_model_id());

                LowerStatement::from_model(self.schema, sub_model)
                    .visit_stmt_query_mut(&mut expr.query);

                let maybe_res = self.lower_expr_binary_op(
                    stmt::BinaryOp::Eq,
                    &mut expr.expr,
                    expr.query.body.as_select_mut().returning.as_expr_mut(),
                );

                assert!(maybe_res.is_none(), "TODO");

                return;
            }
            _ => {
                return;
            }
        };

        if let Some(expr) = maybe_expr {
            *i = expr;
        }
    }

    fn visit_expr_in_subquery_mut(&mut self, i: &mut stmt::ExprInSubquery) {
        self.visit_expr_mut(&mut i.expr);
    }

    fn visit_expr_stmt_mut(&mut self, _i: &mut stmt::ExprStmt) {}

    fn visit_insert_target_mut(&mut self, i: &mut stmt::InsertTarget) {
        *i = stmt::InsertTable {
            table: self.mapping.table,
            columns: self.mapping.columns.clone(),
        }
        .into();
    }

    fn visit_returning_mut(&mut self, i: &mut stmt::Returning) {
        if let stmt::Returning::Star = *i {
            *i = stmt::Returning::Expr(self.mapping.table_to_model.clone().into());
        }

        stmt::visit_mut::visit_returning_mut(self, i);
    }

    fn visit_stmt_delete_mut(&mut self, i: &mut stmt::Delete) {
        stmt::visit_mut::visit_stmt_delete_mut(self, i);

        assert!(i.returning.is_none(), "TODO; stmt={i:#?}");

        // Apply lowering constraint
        self.apply_lowering_filter_constraint(&mut i.filter);
    }

    fn visit_stmt_insert_mut(&mut self, i: &mut stmt::Insert) {
        match &mut i.source.body {
            stmt::ExprSet::Values(values) => {
                for row in &mut values.rows {
                    self.lower_insert_values(row);
                }
            }
            _ => todo!("stmt={i:#?}"),
        }

        stmt::visit_mut::visit_stmt_insert_mut(self, i);
    }

    fn visit_stmt_select_mut(&mut self, i: &mut stmt::Select) {
        stmt::visit_mut::visit_stmt_select_mut(self, i);

        // Apply lowering constraint
        self.apply_lowering_filter_constraint(&mut i.filter);
    }

    fn visit_stmt_update_mut(&mut self, i: &mut stmt::Update) {
        // Before lowering children, convert the "Changed" returning statement
        // to an expression referencing changed fields.

        if let Some(returning) = &mut i.returning {
            if returning.is_changed() {
                let mut fields = vec![];

                for i in i.assignments.keys() {
                    let field = &self.model.fields[i];

                    assert!(field.ty.is_primitive(), "field={field:#?}");

                    fields.push(stmt::Expr::field(app::FieldId {
                        model: self.model.id,
                        index: i,
                    }));
                }

                *returning = stmt::Returning::Expr(stmt::Expr::cast(
                    stmt::ExprRecord::from_vec(fields),
                    stmt::Type::SparseRecord(i.assignments.keys().collect()),
                ));
            }
        }

        stmt::visit_mut::visit_stmt_update_mut(self, i);
    }

    fn visit_source_mut(&mut self, i: &mut stmt::Source) {
        debug_assert!(i.is_model());
        *i = stmt::Source::table(self.table.id);
    }

    fn visit_update_target_mut(&mut self, i: &mut stmt::UpdateTarget) {
        *i = stmt::UpdateTarget::table(self.table.id);
    }
}

impl LowerStatement<'_> {
    fn apply_lowering_filter_constraint(&self, filter: &mut stmt::Expr) {
        // TODO: we really shouldn't have to simplify here, but until
        // simplification includes overlapping predicate pruning, we have to do
        // this here.
        simplify::simplify_expr(self.schema, simplify::ExprTarget::Const, filter);

        let mut operands = vec![];

        for column in self.table.primary_key_columns() {
            let pattern = match &self.mapping.model_to_table[column.id.index] {
                stmt::Expr::ConcatStr(expr) => {
                    // hax
                    let stmt::Expr::Value(stmt::Value::String(a)) = &expr.exprs[0] else {
                        todo!()
                    };
                    let stmt::Expr::Value(stmt::Value::String(b)) = &expr.exprs[1] else {
                        todo!()
                    };

                    format!("{a}{b}")
                }
                stmt::Expr::Value(_) => todo!(),
                _ => continue,
            };

            if is_eq_constrained(filter, column) {
                continue;
            }

            assert_eq!(self.mapping.columns[column.id.index], column.id);

            operands.push(stmt::Expr::begins_with(
                stmt::Expr::column(column.id),
                pattern,
            ));
        }

        if operands.is_empty() {
            return;
        }

        match filter {
            stmt::Expr::And(expr_and) => {
                expr_and.operands.extend(operands);
            }
            expr => {
                operands.push(expr.take());
                *expr = stmt::ExprAnd { operands }.into();
            }
        }
    }

    fn lower_expr_binary_op(
        &mut self,
        op: stmt::BinaryOp,
        lhs: &mut stmt::Expr,
        rhs: &mut stmt::Expr,
    ) -> Option<stmt::Expr> {
        match (&mut *lhs, &mut *rhs) {
            (stmt::Expr::Value(value), other) | (other, stmt::Expr::Value(value))
                if value.is_null() =>
            {
                let other = other.take();

                Some(match op {
                    stmt::BinaryOp::Eq => stmt::Expr::is_null(other),
                    stmt::BinaryOp::Ne => stmt::Expr::is_not_null(other),
                    _ => todo!(),
                })
            }
            (stmt::Expr::DecodeEnum(base, _, variant), other)
            | (other, stmt::Expr::DecodeEnum(base, _, variant)) => {
                assert!(op.is_eq());

                Some(stmt::Expr::eq(
                    (**base).clone(),
                    stmt::Expr::concat_str((
                        variant.to_string(),
                        "#",
                        stmt::Expr::cast(other.clone(), stmt::Type::String),
                    )),
                ))
            }
            (stmt::Expr::Cast(expr_cast), other) if expr_cast.ty.is_id() => {
                Self::uncast_expr_id(lhs);
                Self::uncast_expr_id(other);
                None
            }
            (stmt::Expr::Cast(_), stmt::Expr::Cast(_)) => todo!(),
            _ => None,
        }
    }

    fn lower_expr_in_list(
        &mut self,
        expr: &mut stmt::Expr,
        list: &mut stmt::Expr,
    ) -> Option<stmt::Expr> {
        match (&mut *expr, list) {
            (expr, stmt::Expr::Map(expr_map)) => {
                assert!(expr_map.base.is_arg(), "TODO");
                let maybe_res =
                    self.lower_expr_binary_op(stmt::BinaryOp::Eq, expr, &mut expr_map.map);

                assert!(maybe_res.is_none(), "TODO");
                None
            }
            (stmt::Expr::Cast(expr_cast), list) if expr_cast.ty.is_id() => {
                Self::uncast_expr_id(expr);

                match list {
                    stmt::Expr::List(expr_list) => {
                        for expr in &mut expr_list.items {
                            Self::uncast_expr_id(expr);
                        }
                    }
                    stmt::Expr::Value(stmt::Value::List(items)) => {
                        for item in items {
                            Self::uncast_value_id(item);
                        }
                    }
                    stmt::Expr::Arg(_) => {
                        let arg = list.take();

                        // TODO: don't always cast to a string...
                        let cast = stmt::Expr::cast(stmt::Expr::arg(0), stmt::Type::String);
                        *list = stmt::Expr::map(arg, cast);
                    }
                    _ => todo!("expr={expr:#?}; list={list:#?}"),
                }

                None
            }
            (stmt::Expr::Record(lhs), stmt::Expr::List(list)) => {
                // TODO: implement for real
                for lhs in lhs {
                    assert!(lhs.is_column());
                }

                for item in &mut list.items {
                    assert!(item.is_value());
                }

                None
            }
            (stmt::Expr::Record(lhs), stmt::Expr::Value(stmt::Value::List(_))) => {
                // TODO: implement for real
                for lhs in lhs {
                    assert!(lhs.is_column());
                }

                None
            }
            (expr, list) => todo!("expr={expr:#?}; list={list:#?}"),
        }
    }

    fn lower_insert_values(&self, expr: &mut stmt::Expr) {
        let mut lowered = self.mapping.model_to_table.clone();
        Substitute(&mut *expr).visit_expr_record_mut(&mut lowered);
        *expr = lowered.into();
    }

    fn uncast_expr_id(expr: &mut stmt::Expr) {
        match expr {
            stmt::Expr::Value(value) => {
                Self::uncast_value_id(value);
            }
            stmt::Expr::Cast(expr_cast) if expr_cast.ty.is_id() => {
                *expr = expr_cast.expr.take();
            }
            stmt::Expr::Project(_) => {
                // TODO: don't always cast to a string...
                let base = expr.take();
                *expr = stmt::Expr::cast(base, stmt::Type::String);
            }
            stmt::Expr::List(expr_list) => {
                for expr in &mut expr_list.items {
                    Self::uncast_expr_id(expr);
                }
            }
            _ => todo!("{expr:#?}"),
        }
    }

    fn uncast_value_id(value: &mut stmt::Value) {
        match value {
            stmt::Value::Id(_) => {
                let uncast = value.take().into_id().into_primitive();
                *value = uncast;
            }
            stmt::Value::List(items) => {
                for item in items {
                    Self::uncast_value_id(item);
                }
            }
            _ => todo!("{value:#?}"),
        }
    }
}

impl<I: Input> VisitMut for Substitute<I> {
    fn visit_expr_mut(&mut self, i: &mut stmt::Expr) {
        match i {
            stmt::Expr::Reference(stmt::ExprReference::Field { model, index }) => {
                let field_id = FieldId {
                    model: *model,
                    index: *index,
                };
                *i = self.0.resolve_field(field_id);
            }
            // Do not traverse these
            stmt::Expr::InSubquery(_) | stmt::Expr::Stmt(_) => {}
            _ => {
                // Traverse other expressions
                stmt::visit_mut::visit_expr_mut(self, i);
            }
        }
    }

    fn visit_expr_in_subquery_mut(&mut self, _i: &mut stmt::ExprInSubquery) {
        todo!()
    }
    fn visit_expr_stmt_mut(&mut self, _i: &mut stmt::ExprStmt) {
        todo!()
    }
}

impl Input for &mut stmt::Expr {
    fn resolve_field(&mut self, field_id: FieldId) -> stmt::Expr {
        self.entry(field_id.index).to_expr()
    }
}

impl Input for &stmt::Assignments {
    fn resolve_field(&mut self, field_id: FieldId) -> stmt::Expr {
        let assignment = &self[field_id.index];
        assert!(assignment.op.is_set());
        assignment.expr.clone()
    }
}

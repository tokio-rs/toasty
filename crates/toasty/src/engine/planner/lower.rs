use super::*;

use db::{Column, Table};

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
    fn resolve_field(&mut self, expr_field: &stmt::ExprField) -> stmt::Expr;
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
    pub(crate) fn lower_stmt_delete(&self, model: &app::Model, stmt: &mut stmt::Delete) {
        LowerStatement::from_model(self.schema, model).visit_stmt_delete_mut(stmt);
        simplify::simplify_stmt(self.schema, stmt);
    }

    pub(crate) fn lower_stmt_query(&self, model: &app::Model, stmt: &mut stmt::Query) {
        LowerStatement::from_model(self.schema, model).visit_stmt_query_mut(stmt);
        simplify::simplify_stmt(self.schema, stmt);
    }

    pub(crate) fn lower_stmt_insert(&self, model: &app::Model, stmt: &mut stmt::Insert) {
        LowerStatement::from_model(self.schema, model).visit_stmt_insert_mut(stmt);
        simplify::simplify_stmt(self.schema, stmt);
    }

    pub(crate) fn lower_stmt_update(&self, model: &app::Model, stmt: &mut stmt::Update) {
        LowerStatement::from_model(self.schema, model).visit_stmt_update_mut(stmt);
        simplify::simplify_stmt(self.schema, stmt);
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
            stmt::Expr::Field(expr) => {
                *i = self.mapping.table_to_model[expr.field.index].clone();

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
            stmt::Expr::Field(expr_field) => {
                *i = self.0.resolve_field(expr_field);
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
    fn resolve_field(&mut self, expr_field: &stmt::ExprField) -> stmt::Expr {
        self.entry(expr_field.field.index).to_expr()
    }
}

impl Input for &stmt::Assignments {
    fn resolve_field(&mut self, expr_field: &stmt::ExprField) -> stmt::Expr {
        let assignment = &self[expr_field.field.index];
        assert!(assignment.op.is_set());
        assignment.expr.clone()
    }
}

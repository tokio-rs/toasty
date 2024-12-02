use std::ops::Sub;

use stmt::substitute;

use super::*;

struct LowerStatement<'a> {
    schema: &'a Schema,

    /// The model in which the statement is contextualized.
    model: &'a Model,

    /// The associated table for the model.
    table: &'a Table,
}

/// Substitute fields for columns
struct Substitute<I>(I);

trait Input {
    fn resolve_field(&mut self, expr_field: &stmt::ExprField) -> stmt::Expr;
}

impl<'a> LowerStatement<'a> {
    fn from_model(schema: &'a Schema, model: &'a Model) -> LowerStatement<'a> {
        LowerStatement {
            schema,
            model,
            table: schema.table(model.lowering.table),
        }
    }
}

impl Planner<'_> {
    pub(crate) fn lower_stmt_delete(&self, model: &Model, stmt: &mut stmt::Delete) {
        LowerStatement::from_model(self.schema, model).visit_stmt_delete_mut(stmt);
    }

    pub(crate) fn lower_stmt_query(&self, model: &Model, stmt: &mut stmt::Query) {
        LowerStatement::from_model(self.schema, model).visit_stmt_query_mut(stmt);
    }

    pub(crate) fn lower_stmt_insert(&self, model: &Model, stmt: &mut stmt::Insert) {
        LowerStatement::from_model(self.schema, model).visit_stmt_insert_mut(stmt);
    }

    pub(crate) fn lower_stmt_update(&self, model: &Model, stmt: &mut stmt::Update) {
        LowerStatement::from_model(self.schema, model).visit_stmt_update_mut(stmt);
    }

    // TODO: get rid of this
    pub(crate) fn lower_stmt_filter(&self, model: &Model, filter: &mut stmt::Expr) {
        let mut lower = LowerStatement::from_model(self.schema, model);
        lower.visit_expr_mut(filter);
        lower.apply_lowering_filter_constraint(filter);
    }
}

fn is_constrained(expr: &stmt::Expr, column: &Column) -> bool {
    match expr {
        stmt::Expr::And(expr) => expr.iter().any(|expr| is_constrained(expr, column)),
        stmt::Expr::Or(expr) => expr.iter().all(|expr| is_constrained(expr, column)),
        stmt::Expr::BinaryOp(expr) => is_constrained(&*expr.lhs, column),
        stmt::Expr::Project(expr) => expr.projection.resolves_to(column.id),
        stmt::Expr::Record(lhs) => lhs.fields.iter().any(|expr| is_constrained(expr, column)),
        stmt::Expr::InList(expr) => is_constrained(&*expr.expr, column),
        _ => todo!("expr={:#?}", expr),
    }
}

impl<'a> VisitMut for LowerStatement<'a> {
    fn visit_assignments_mut(&mut self, i: &mut stmt::Assignments) {
        let mut assignments = stmt::Assignments::default();

        for (index, update_expr) in i.iter() {
            let field = &self.model.fields[index];

            if field.primary_key {
                todo!("updating PK not supported yet");
            }

            match &field.ty {
                FieldTy::Primitive(primitive) => {
                    let mut lowered =
                        self.model.lowering.model_to_table[primitive.lowering].clone();
                    Substitute(&*i).visit_expr_mut(&mut lowered);
                    // lowered.simplify();
                    todo!();
                    assignments.set(primitive.column, lowered);
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
                *i = self.model.lowering.table_to_model[expr.field.index].clone();

                self.visit_expr_mut(i);
                return;
            }
            stmt::Expr::InList(expr) => self.lower_expr_in_list(&mut expr.expr, &mut expr.list),
            stmt::Expr::InSubquery(expr) => {
                let sub_model = self
                    .schema
                    .model(expr.query.body.as_select().source.as_model_id());

                LowerStatement::from_model(&self.schema, sub_model)
                    .visit_stmt_query_mut(&mut expr.query);

                let maybe_res = self.lower_expr_binary_op(
                    stmt::BinaryOp::Eq,
                    &mut expr.expr,
                    expr.query.body.as_select_mut().returning.as_expr_mut(),
                );

                assert!(maybe_res.is_none(), "TODO");
                todo!("stmt={expr:#?}");

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

    fn visit_expr_stmt_mut(&mut self, i: &mut stmt::ExprStmt) {}

    fn visit_insert_target_mut(&mut self, i: &mut stmt::InsertTarget) {
        *i = stmt::InsertTable {
            table: self.model.lowering.table,
            columns: self.model.lowering.columns.clone(),
        }
        .into();
    }

    fn visit_returning_mut(&mut self, i: &mut stmt::Returning) {
        match i {
            stmt::Returning::Star => {
                // Swap returning for an already lowered expression
                *i = stmt::Returning::Expr(self.model.lowering.table_to_model.clone().into());
            }
            stmt::Returning::Expr(returning) => {
                self.visit_expr_mut(returning);
            }
            _ => todo!("stmt={i:#?}"),
        }
    }

    fn visit_stmt_delete_mut(&mut self, i: &mut stmt::Delete) {
        stmt::visit_mut::visit_stmt_delete_mut(self, i);

        assert!(i.returning.is_none(), "TODO; stmt={i:#?}");

        // Apply lowering constraint
        self.apply_lowering_filter_constraint(&mut i.filter);
    }

    fn visit_stmt_insert_mut(&mut self, i: &mut stmt::Insert) {
        match &mut *i.source.body {
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

                for i in i.assignments.fields.iter() {
                    let i = i.into_usize();
                    let field = &self.model.fields[i];

                    assert!(field.ty.is_primitive(), "field={field:#?}");

                    fields.push(stmt::Expr::field(FieldId {
                        model: self.model.id,
                        index: i,
                    }));
                }

                *returning = stmt::Returning::Expr(stmt::ExprRecord::from_vec(fields).into());
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

    fn visit_value_mut(&mut self, i: &mut stmt::Value) {
        // if let stmt::Value::Id(id) = i {
        //     *i = id.to_primitive();
        // }
    }
}

impl<'a> LowerStatement<'a> {
    fn apply_lowering_filter_constraint(&self, filter: &mut stmt::Expr) {
        use std::mem;

        let mut expr = mem::take(filter);

        // Include any column constraints that are constant as part of the
        // lowering.
        let mut operands = match expr {
            stmt::Expr::And(expr_and) => expr_and.operands,
            expr => vec![expr],
        };

        for column in self.table.primary_key_columns() {
            // TODO: don't hard code
            let pattern = match &self.model.lowering.model_to_table[column.id.index] {
                stmt::Expr::ConcatStr(expr) => {
                    // hax
                    let stmt::Expr::Value(stmt::Value::String(a)) = &expr.exprs[0] else {
                        todo!()
                    };
                    let stmt::Expr::Value(stmt::Value::String(b)) = &expr.exprs[1] else {
                        todo!()
                    };

                    format!("{}{}", a, b)
                }
                stmt::Expr::Value(value) => todo!(),
                _ => continue,
            };

            assert_eq!(self.model.lowering.columns[column.id.index], column.id);

            operands.push(stmt::Expr::begins_with(stmt::Expr::column(column), pattern));
        }

        *filter = if operands.len() == 1 {
            operands.into_iter().next().unwrap()
        } else {
            stmt::ExprAnd { operands: operands }.into()
        };
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
                assert!(!other.is_cast(), "{other:#?}");

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
                    stmt::Expr::concat_str((variant.to_string(), "#", other.clone())),
                ))
            }
            (stmt::Expr::Cast(expr_cast), other) if expr_cast.ty.is_id() => {
                self.uncast_id(lhs);
                self.uncast_id(other);
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
                self.uncast_id(expr);

                match list {
                    stmt::Expr::List(expr_list) => {
                        for expr in expr_list {
                            self.uncast_id(expr);
                        }
                    }
                    _ => todo!("list={list:#?}"),
                }

                None
            }
            (expr, list) => todo!("expr={expr:#?}; list={list:#?}"),
        }
    }

    fn lower_insert_values(&self, expr: &mut stmt::Expr) {
        let stmt::Expr::Record(row) = expr else {
            todo!()
        };

        let mut lowered = self.model.lowering.model_to_table.clone();
        Substitute(&mut *row).visit_expr_record_mut(&mut lowered);
        *expr = lowered.into();

        simplify::simplify_expr(self.schema, self.table, expr);
        todo!("expr={expr:#?}");
    }

    fn uncast_id(&self, expr: &mut stmt::Expr) {
        match expr {
            stmt::Expr::Value(value) if value.is_id() => {
                let value = expr.take().into_value().into_id().into_primitive();
                *expr = value.into();
            }
            stmt::Expr::Cast(expr_cast) if expr_cast.ty.is_id() => {
                *expr = expr_cast.expr.take();
            }
            stmt::Expr::Project(_) => {
                // TODO: don't always cast to a string...
                let base = expr.take();
                *expr = stmt::Expr::cast(base, stmt::Type::String);
            }
            _ => todo!("{expr:#?}"),
        }
    }
}

impl<I: Input> VisitMut for Substitute<I> {
    fn visit_expr_mut(&mut self, i: &mut stmt::Expr) {
        match i {
            stmt::Expr::Field(expr_field) => {
                *i = self.0.resolve_field(&expr_field);
            }
            // Do not traverse these
            stmt::Expr::InSubquery(_) | stmt::Expr::Stmt(_) => {}
            _ => {
                // Traverse other expressions
                stmt::visit_mut::visit_expr_mut(self, i);
            }
        }
    }

    fn visit_expr_in_subquery_mut(&mut self, i: &mut stmt::ExprInSubquery) {
        todo!()
    }
    fn visit_expr_stmt_mut(&mut self, i: &mut stmt::ExprStmt) {
        todo!()
    }
}

impl Input for &mut stmt::ExprRecord {
    fn resolve_field(&mut self, expr_field: &stmt::ExprField) -> stmt::Expr {
        self[expr_field.field.index].clone()
    }
}

impl Input for &stmt::Assignments {
    fn resolve_field(&mut self, expr_field: &stmt::ExprField) -> stmt::Expr {
        self[expr_field.field.index].clone()
    }
}

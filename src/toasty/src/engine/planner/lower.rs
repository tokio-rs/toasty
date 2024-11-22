use super::*;

impl Planner<'_> {
    pub(crate) fn lower_stmt_delete(&self, model: &Model, stmt: &mut stmt::Delete) {
        let table = self.schema.table(model.lowering.table);

        // Lower the query source
        stmt.from = stmt::Source::table(table.id);

        self.lower_stmt_filter(table, model, &mut stmt.filter);

        assert!(stmt.returning.is_none(), "TODO; stmt={stmt:#?}");
    }

    pub(crate) fn lower_stmt_query(&self, table: &Table, model: &Model, stmt: &mut stmt::Query) {
        match &mut *stmt.body {
            stmt::ExprSet::Select(stmt) => self.lower_stmt_select(table, model, stmt),
            _ => todo!("stmt={stmt:#?}"),
        }
    }

    pub(crate) fn lower_returning(&self, model: &Model, stmt: &mut stmt::Returning) {
        match stmt {
            stmt::Returning::Star => {
                let mut returning: stmt::Expr = model.lowering.table_to_model.clone().into();
                returning.substitute(stmt::substitute::ModelToTable(model));

                *stmt = stmt::Returning::Expr(returning);
            }
            stmt::Returning::Expr(returning) => {
                returning.substitute(stmt::substitute::ModelToTable(model));
            }
            _ => todo!(),
        }
    }

    fn lower_stmt_select(&self, table: &Table, model: &Model, stmt: &mut stmt::Select) {
        use std::mem;

        // Lower the query source
        stmt.source = stmt::Source::table(table.id);

        // Lower the selection filter
        self.lower_stmt_filter(table, model, &mut stmt.filter);

        self.lower_returning(model, &mut stmt.returning);
    }

    /// Lower the filter portion of a statement
    pub(crate) fn lower_stmt_filter(&self, table: &Table, model: &Model, filter: &mut stmt::Expr) {
        use std::mem;

        let mut expr = mem::take(filter);

        // Lower the filter
        expr.substitute(stmt::substitute::ModelToTable(model));

        // Include any column constraints that are constant as part of the
        // lowering.
        let mut operands = match expr {
            stmt::Expr::And(expr_and) => expr_and.operands,
            expr => vec![expr],
        };

        for column in table.primary_key_columns() {
            // TODO: don't hard code
            let pattern = match &model.lowering.model_to_table[column.id.index] {
                // stmt::Expr::Enum(expr_enum) => expr_enum,
                // stmt::Expr::DecodeEnum(..) => todo!(),
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

            assert_eq!(model.lowering.columns[column.id.index], column.id);

            operands.push(stmt::Expr::begins_with(stmt::Expr::column(column), pattern));
        }

        *filter = if operands.len() == 1 {
            operands.into_iter().next().unwrap()
        } else {
            stmt::ExprAnd { operands: operands }.into()
        };

        self.lower_expr(filter);
    }

    pub(crate) fn lower_stmt_condition(
        &self,
        table: &Table,
        model: &Model,
        condition: &mut stmt::Expr,
    ) {
        // Lower the filter
        condition.substitute(stmt::substitute::ModelToTable(model));
        self.lower_expr(condition);
    }

    pub(crate) fn lower_insert_expr(&self, model: &Model, expr: &mut stmt::Expr) {
        let stmt::Expr::Record(record) = expr else {
            todo!()
        };

        let mut lowered = vec![];

        for lowering in &model.lowering.model_to_table {
            let mut lowering = lowering.clone();
            lowering.substitute(stmt::substitute::ModelToTable(&*record));
            self.lower_expr(&mut lowering);
            lowered.push(lowering);
        }

        *expr = stmt::ExprRecord::from_vec(lowered).into();
    }

    pub(crate) fn lower_update_stmt(&self, model: &Model, stmt: &mut stmt::Update) {
        let table = self.schema.table(model.lowering.table);

        stmt.target = stmt::UpdateTarget::table(table.id);

        // Lower returning first so `Returning::Changed` can be handled.
        if let Some(returning) = &mut stmt.returning {
            if returning.is_changed() {
                let mut fields = vec![];

                for i in stmt.assignments.fields.iter() {
                    let i = i.into_usize();
                    let field = &model.fields[i];

                    assert!(field.ty.is_primitive(), "field={field:#?}");

                    fields.push(stmt::Expr::field(FieldId {
                        model: model.id,
                        index: i,
                    }));
                }

                *returning = stmt::Returning::Expr(stmt::ExprRecord::from_vec(fields).into());
            }

            self.lower_returning(model, returning);
        }

        let mut assignments = stmt::Assignments::default();

        for (index, update_expr) in stmt.assignments.iter() {
            let field = &model.fields[index];

            if field.primary_key {
                todo!("updating PK not supported yet");
            }

            match &field.ty {
                FieldTy::Primitive(primitive) => {
                    let mut lowered = model.lowering.model_to_table[primitive.lowering].clone();
                    lowered.substitute(stmt::substitute::ModelToTable((field.id, &*update_expr)));
                    assignments.set(primitive.column, lowered);
                }
                _ => {
                    todo!("field = {:#?};", field);
                }
            }
        }

        stmt.assignments = assignments;

        if let Some(filter) = &mut stmt.filter {
            self.lower_stmt_filter(table, model, filter);
        }

        if let Some(condition) = &mut stmt.condition {
            self.lower_stmt_condition(table, model, condition);
        }
    }

    pub(crate) fn lower_expr(&self, expr: &mut stmt::Expr) {
        println!("lower_expr = {expr:#?}");
        LowerExpr {}.visit_mut(expr);
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

struct LowerExpr {}

impl LowerExpr {
    fn lower_expr_binary_op(
        &mut self,
        op: stmt::BinaryOp,
        lhs: &mut stmt::Expr,
        rhs: &mut stmt::Expr,
    ) -> Option<stmt::Expr> {
        match (&mut *lhs, &mut *rhs) {
            (stmt::Expr::DecodeEnum(base, _, variant), other)
            | (other, stmt::Expr::DecodeEnum(base, _, variant)) => {
                assert!(op.is_eq());

                Some(stmt::Expr::eq(
                    (**base).clone(),
                    stmt::Expr::concat_str((variant.to_string(), "#", other.clone())),
                ))
            }
            (stmt::Expr::Cast(expr_cast), other) if expr_cast.ty.is_id() => {
                // TODO: don't hard code this cast... and probably recurse
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
                        println!("list = {expr_list:#?}");
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

    fn uncast_id(&self, expr: &mut stmt::Expr) {
        match expr {
            stmt::Expr::Value(value) if value.is_id() => {
                let value = expr.take().into_value().into_id().into_primitive();
                *expr = value.into();
            }
            stmt::Expr::Cast(expr_cast) if expr_cast.ty.is_id() => {
                *expr = expr_cast.expr.take();
            }
            _ => todo!("{expr:#?}"),
        }
    }
}

impl VisitMut for LowerExpr {
    fn visit_expr_mut(&mut self, i: &mut stmt::Expr) {
        stmt::visit_mut::visit_expr_mut(self, i);

        let maybe_expr = match i {
            stmt::Expr::BinaryOp(expr) => {
                self.lower_expr_binary_op(expr.op, &mut expr.lhs, &mut expr.rhs)
            }
            stmt::Expr::InList(expr) => self.lower_expr_in_list(&mut expr.expr, &mut expr.list),
            _ => return,
        };

        if let Some(expr) = maybe_expr {
            *i = expr;
        }
    }

    /*
    fn visit_value_mut(&mut self, i: &mut stmt::Value) {
        stmt::visit_mut::visit_value_mut(self, i);

        match i {
            stmt::Value::Id(value) => {
                *i = value.to_primitive();
            }
            _ => {}
        }
    }
    */
}

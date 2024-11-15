use super::*;

impl<'stmt> Planner<'_, 'stmt> {
    pub(crate) fn lower_stmt_delete(&self, model: &Model, stmt: &mut stmt::Delete<'stmt>) {
        let table = self.schema.table(model.lowering.table);

        // Lower the query source
        stmt.from = stmt::Source::table(table.id);

        self.lower_stmt_filter(table, model, &mut stmt.filter);

        assert!(stmt.returning.is_none(), "TODO; stmt={stmt:#?}");
    }

    pub(crate) fn lower_stmt_query(
        &self,
        table: &Table,
        model: &Model,
        stmt: &mut stmt::Query<'stmt>,
    ) {
        match &mut *stmt.body {
            stmt::ExprSet::Select(stmt) => self.lower_stmt_select(table, model, stmt),
            _ => todo!("stmt={stmt:#?}"),
        }
    }

    pub(crate) fn lower_returning(&self, model: &Model, stmt: &mut stmt::Returning<'stmt>) {
        println!("LOWER_RETURNING = {stmt:#?}");
        match stmt {
            stmt::Returning::Star => {
                let mut returning: stmt::Expr<'_> = model.lowering.table_to_model.clone().into();
                returning.substitute(stmt::substitute::ModelToTable(model));

                *stmt = stmt::Returning::Expr(returning);
            }
            stmt::Returning::Expr(returning) => {
                returning.substitute(stmt::substitute::ModelToTable(model));
            }
            _ => todo!(),
        }
    }

    fn lower_stmt_select(&self, table: &Table, model: &Model, stmt: &mut stmt::Select<'stmt>) {
        use std::mem;

        // Lower the query source
        stmt.source = stmt::Source::table(table.id);

        // Lower the selection filter
        self.lower_stmt_filter(table, model, &mut stmt.filter);

        self.lower_returning(model, &mut stmt.returning);
    }

    /// Lower the filter portion of a statement
    fn lower_stmt_filter(&self, table: &Table, model: &Model, filter: &mut stmt::Expr<'stmt>) {
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
            let expr_enum = match &model.lowering.model_to_table[column.id.index] {
                stmt::Expr::Enum(expr_enum) => expr_enum,
                _ => continue,
            };

            assert_eq!(model.lowering.columns[column.id.index], column.id);

            operands.push(stmt::Expr::is_a(
                stmt::Expr::column(column),
                stmt::ExprTy {
                    ty: column.ty.clone(),
                    variant: Some(expr_enum.variant),
                },
            ))
        }

        *filter = if operands.len() == 1 {
            operands.into_iter().next().unwrap()
        } else {
            stmt::ExprAnd { operands: operands }.into()
        };

        self.lower_expr(filter);
    }

    pub(crate) fn lower_insert_expr(&self, model: &Model, expr: &mut stmt::Expr<'stmt>) {
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

    pub(crate) fn lower_update_stmt(&self, model: &Model, stmt: &mut stmt::Update<'stmt>) {
        let table = self.schema.table(model.lowering.table);

        stmt.target = stmt::UpdateTarget::table(table.id);

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
            // self.lower_expr2(model, condition);
            todo!("condition={condition:#?}");
        }

        if let Some(returning) = &mut stmt.returning {
            self.lower_returning(model, returning);
        }
    }

    pub(crate) fn lower_expr(&self, expr: &mut stmt::Expr<'stmt>) {
        LowerExpr {}.visit_mut(expr);
    }

    /*
    pub(crate) fn lower_expr2(&self, model: &Model, expr: &mut stmt::Expr<'stmt>) {
        LowerExpr2 {
            schema: self.schema,
            model,
        }
        .visit_mut(expr);
    }
    */

    pub(crate) fn lower_index_filter(
        &self,
        table: &Table,
        model: &Model,
        model_index: &ModelIndex,
        expr: &mut stmt::Expr<'stmt>,
    ) {
        use std::mem;

        let lowering = &model_index.lowering;
        let index = &table.indices[lowering.index.index];

        // self.lower_expr2(model, expr);
        todo!();

        // Lets try something...
        let mut operands = vec![mem::take(expr)];

        for index_column in &index.columns {
            let column = self.schema.column(index_column.column);

            // If the expression is already constraining the index column, we
            // don't need to add an additional type constraint.
            if is_constrained(&operands[0], column) {
                continue;
            }

            let expr_enum = match &model.lowering.model_to_table[column.id.index] {
                stmt::Expr::Enum(expr_enum) => expr_enum,
                _ => continue,
            };

            operands.push(stmt::Expr::is_a(
                stmt::Expr::column(column),
                stmt::ExprTy {
                    ty: column.ty.clone(),
                    variant: Some(expr_enum.variant),
                },
            ))
        }

        *expr = if operands.len() == 1 {
            operands.into_iter().next().unwrap()
        } else {
            stmt::ExprAnd { operands: operands }.into()
        };
    }
}

fn is_constrained(expr: &stmt::Expr<'_>, column: &Column) -> bool {
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

impl<'stmt> VisitMut<'stmt> for LowerExpr {
    fn visit_expr_mut(&mut self, i: &mut stmt::Expr<'stmt>) {
        stmt::visit_mut::visit_expr_mut(self, i);

        match i {
            stmt::Expr::Cast(expr) => {
                if expr.ty.is_id() {
                    // Get rid of the cast
                    *i = expr.expr.take();
                }
            }
            _ => {}
        }
    }

    fn visit_value_mut(&mut self, i: &mut stmt::Value<'stmt>) {
        stmt::visit_mut::visit_value_mut(self, i);

        match i {
            stmt::Value::Id(value) => {
                *i = value.to_primitive();
            }
            _ => {}
        }
    }
}

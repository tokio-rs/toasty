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
        match stmt {
            stmt::Returning::Star => {
                let mut returning: stmt::Expr<'_> = model.lowering.table_to_model.clone().into();
                returning.substitute(stmt::substitute::ModelToTable(model));

                *stmt = stmt::Returning::Expr(returning);
            }
            stmt::Returning::Expr(returning) => {
                returning.substitute(stmt::substitute::ModelToTable(model));
            }
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
        // self.lower_expr2(model, &mut expr);
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
    }

    pub(crate) fn lower_insert_expr(&self, model: &Model, expr: &mut stmt::Expr<'stmt>) {
        self.lower_expr2(model, expr);

        let stmt::Expr::Record(record) = expr else {
            todo!()
        };

        let mut lowered = vec![];

        for lowering in &model.lowering.model_to_table {
            let mut lowering = lowering.clone();
            lowering.substitute(stmt::substitute::ModelToTable(&*record));
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
                    assignments.set(index, lowered);
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

    pub(crate) fn lower_expr2(&self, model: &Model, expr: &mut stmt::Expr<'stmt>) {
        LowerExpr2 {
            schema: self.schema,
            model,
        }
        .visit_mut(expr);
    }

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

        self.lower_expr2(model, expr);

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

struct LowerExpr2<'a> {
    schema: &'a Schema,
    model: &'a Model,
}

impl<'a, 'stmt> VisitMut<'stmt> for LowerExpr2<'a> {
    fn visit_expr_mut(&mut self, i: &mut stmt::Expr<'stmt>) {
        stmt::visit_mut::visit_expr_mut(self, i);

        match i {
            stmt::Expr::Field(expr) => {
                *i = self.model.lowering.model_to_table[expr.field.index].clone();
            }
            _ => {}
        }
    }

    /*
    fn visit_expr_binary_op_mut(&mut self, i: &mut stmt::ExprBinaryOp<'stmt>) {
        use stmt::Expr::*;

        match (&mut *i.lhs, &mut *i.rhs) {
            (Field(lhs), rhs) => {
                assert!(i.op.is_eq(), "op={:#?}", i.op);
                let lowered_lhs = self.lower_field_expr(lhs.field, rhs);
                *i.lhs = lowered_lhs;
            }
            (lhs, Field(rhs)) => {
                assert!(i.op.is_eq(), "op={:#?}", i.op);
                let lowered_rhs = self.lower_field_expr(rhs.field, lhs);
                *i.rhs = lowered_rhs;
            }
            _ => todo!("expr = {:#?}", i),
        }

        stmt::visit_mut::visit_expr_binary_op_mut(self, i);
    }
    */

    fn visit_expr_in_list_mut(&mut self, i: &mut stmt::ExprInList<'stmt>) {
        /*
        use stmt::Expr::*;

        match (&mut *i.expr, &mut *i.list) {
            (Project(lhs), rhs) => {
                // self.lower_expr(&mut lhs.projection, rhs);
                todo!()
            }
            (Record(lhs), List(_)) => {
                // TODO: implement for real
                for lhs in lhs {
                    let Project(expr_project) = lhs else {
                        todo!("expr={:#?}", i)
                    };
                    let field = expr_project
                        .projection
                        .resolve_field(self.schema, self.model);
                    let lowering = field.ty.expect_primitive().lowering;
                    assert_eq!(*lhs, self.model.lowering.table_to_model[lowering]);
                }
            }
            _ => todo!("expr={:#?}", i),
        }
        */
        todo!()
    }

    fn visit_expr_in_subquery_mut(&mut self, i: &mut stmt::ExprInSubquery<'stmt>) {
        /*
        stmt::visit_mut::visit_expr_mut(self, &mut *i.expr);
        stmt::visit_mut::visit_stmt_query_mut(self, &mut *i.query);

        let stmt::ExprSet::Select(select) = &mut *i.query.body else {
            todo!()
        };

        let model = self.schema.model(select.source.as_model_id());

        let stmt::Returning::Expr(returning) = &mut select.returning else {
            todo!()
        };
        /*
        returning.substitute(stmt::substitute::TableToModel(
            &model.lowering.table_to_model,
        ));
        */
        todo!()

        // TODO: do the rest of the lowering...
        */
        todo!()
    }
}

impl<'a> LowerExpr2<'a> {
    fn lower_field_expr<'stmt>(
        &mut self,
        field_id: FieldId,
        expr: &mut stmt::Expr<'stmt>,
    ) -> stmt::Expr<'stmt> {
        // This is an input for a targeted substitution. Only a single
        // projection should be substituted here
        struct Input<'a, 'stmt> {
            field_id: FieldId,
            expr: &'a stmt::Expr<'stmt>,
        }

        impl<'a, 'stmt> stmt::substitute::Input<'stmt> for Input<'a, 'stmt> {
            fn resolve_field(&mut self, expr_field: &stmt::ExprField) -> Option<stmt::Expr<'stmt>> {
                assert_eq!(expr_field.field, self.field_id);
                Some(self.expr.clone())
            }
        }

        // Find the referenced model field.
        let field = self.schema.field(field_id);

        // Column the field is mapped to
        let lowering_idx = match &field.ty {
            FieldTy::Primitive(primitive) => primitive.lowering,
            _ => todo!("field = {:#?}; expr={:#?}", field, expr),
        };

        let mut lowered = self.model.lowering.model_to_table[lowering_idx].clone();
        lowered.substitute(Input {
            field_id,
            expr: &*expr,
        });

        stmt::Expr::column(self.model.lowering.columns[lowering_idx])
    }
}

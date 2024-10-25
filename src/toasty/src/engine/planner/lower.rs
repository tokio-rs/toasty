use super::*;

impl<'stmt> Planner<'_, 'stmt> {
    pub(crate) fn lower_delete_stmt(&self, model: &Model, stmt: &mut stmt::Delete<'stmt>) {
        /*
        let mut filter = stmt.selection.body.as_select().filter.clone();

        self.lower_expr2(model, &mut filter);

        let sql = sql::Statement::delete(self.schema, model.lowering.table, filter);
        */
        todo!()
    }

    /*
    pub(crate) fn lower_insert_expr(
        &self,
        model: &Model,
        mut expr: stmt::Expr<'stmt>,
    ) -> Vec<sql::Expr<'stmt>> {
        self.lower_expr2(model, &mut expr);

        let record = match expr {
            stmt::Expr::Record(record) => record,
            _ => todo!(),
        };

        let mut lowered = vec![];

        for lowering in &model.lowering.model_to_table {
            let mut lowering = lowering.clone();
            lowering.substitute(stmt::substitute::ModelToTable(&record));
            lowered.push(sql::Expr::from_stmt(
                &self.schema,
                model.lowering.table,
                lowering,
            ));
        }

        lowered
    }
    */

    /*
    pub(crate) fn lower_update_expr(
        &self,
        model: &Model,
        stmt: &stmt::Update<'stmt>,
    ) -> sql::Update<'stmt> {
        // TODO: handle
        let mut returning = vec![];

        let mut assignments = vec![];

        for updated_field in stmt.fields.iter() {
            let field = &model.fields[updated_field.into_usize()];

            if field.primary_key {
                todo!("updating PK not supported yet");
            }

            match &field.ty {
                FieldTy::Primitive(primitive) => {
                    let mut lowered = model.lowering.model_to_table[primitive.lowering].clone();
                    lowered.substitute(stmt::substitute::ModelToTable(&stmt.expr));

                    assignments.push(sql::Assignment {
                        target: primitive.column,
                        value: sql::Expr::from_stmt(self.schema, model.lowering.table, lowered),
                    });

                    if stmt.returning {
                        returning.push(sql::Expr::Column(primitive.column));
                    }
                }
                _ => {
                    todo!("field = {:#?}; stmt={:#?}", field, stmt);
                }
            }
        }

        let mut selection = stmt.selection.body.as_select().filter.clone();

        self.lower_expr2(model, &mut selection);

        let pre_condition = match &stmt.condition {
            Some(expr) => {
                let mut expr = expr.clone();
                self.lower_expr2(model, &mut expr);
                Some(sql::Expr::from_stmt(
                    self.schema,
                    model.lowering.table,
                    expr,
                ))
            }
            None => None,
        };

        sql::Update {
            assignments,
            table: sql::TableWithJoins {
                table: model.lowering.table,
                alias: 0,
            },
            selection: Some(sql::Expr::from_stmt(
                self.schema,
                model.lowering.table,
                selection,
            )),
            pre_condition: pre_condition,
            returning: if stmt.returning {
                Some(returning)
            } else {
                None
            },
        }
    }
    */

    pub(crate) fn lower_expr2(&self, model: &Model, expr: &mut stmt::Expr<'stmt>) {
        LowerExpr2 {
            schema: self.schema,
            model,
        }
        .visit_mut(expr);
    }

    pub(crate) fn lower_select(&self, table: &Table, model: &Model, expr: &mut stmt::Expr<'stmt>) {
        use std::mem;

        self.lower_expr2(model, expr);

        // Lets try something...
        let mut operands = match mem::take(expr) {
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

        *expr = if operands.len() == 1 {
            operands.into_iter().next().unwrap()
        } else {
            stmt::ExprAnd { operands: operands }.into()
        };
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
    /*
    TODO: lowering here requires prefixing
    fn visit_expr_mut(&mut self, i: &mut stmt::Expr<'stmt>) {
        stmt::visit_mut::visit_expr_mut(self, &mut *i);

        match i {
            stmt::Expr::Enum(expr_enum) => {
                // TODO: optimize
                if expr_enum.fields.len() == 1 {
                    *i = expr_enum.fields[0].clone();
                } else {
                    *i = expr_enum.fields.clone().into();
                }
            }
            _ => {}
        }
    }
    */

    fn visit_expr_binary_op_mut(&mut self, i: &mut stmt::ExprBinaryOp<'stmt>) {
        use stmt::Expr::*;

        match (&mut *i.lhs, &mut *i.rhs) {
            (Project(lhs), rhs) => {
                self.lower_expr(&mut lhs.projection, rhs);
            }
            (lhs, Project(rhs)) => {
                assert!(i.op.is_eq(), "op={:#?}", i.op);
                self.lower_expr(&mut rhs.projection, lhs);
            }
            _ => todo!("expr = {:#?}", i),
        }

        stmt::visit_mut::visit_expr_binary_op_mut(self, i);
    }

    fn visit_expr_in_list_mut(&mut self, i: &mut stmt::ExprInList<'stmt>) {
        use stmt::Expr::*;

        match (&mut *i.expr, &mut *i.list) {
            (Project(lhs), rhs) => {
                self.lower_expr(&mut lhs.projection, rhs);
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
    }

    fn visit_expr_in_subquery_mut(&mut self, i: &mut stmt::ExprInSubquery<'stmt>) {
        stmt::visit_mut::visit_expr_mut(self, &mut *i.expr);
        stmt::visit_mut::visit_stmt_query_mut(self, &mut *i.query);

        let stmt::ExprSet::Select(select) = &mut *i.query.body else {
            todo!()
        };

        let model = self.schema.model(select.source.as_model_id());

        let stmt::Returning::Expr(returning) = &mut select.returning else {
            todo!()
        };
        returning.substitute(stmt::substitute::TableToModel(
            &model.lowering.table_to_model,
        ));

        // TODO: do the rest of the lowering...
    }
}

impl<'a> LowerExpr2<'a> {
    fn lower_expr<'stmt>(
        &mut self,
        projection: &mut stmt::Projection,
        expr: &mut stmt::Expr<'stmt>,
    ) {
        // This is an input for a targeted substitution. Only a single
        // projection should be substituted here
        struct Input<'a, 'stmt> {
            projection: &'a stmt::Projection,
            expr: &'a mut stmt::Expr<'stmt>,
        }

        impl<'a, 'stmt> stmt::substitute::Input<'stmt> for Input<'a, 'stmt> {}

        // Find the referenced model field.
        let field = projection.resolve_field(self.schema, self.model);

        // Column the field is mapped to
        let lowering_idx = match &field.ty {
            FieldTy::Primitive(primitive) => primitive.lowering,
            _ => todo!(
                "field = {:#?}; projection={:#?}; expr={:#?}",
                field,
                projection,
                expr
            ),
        };

        let mut lowered = self.model.lowering.model_to_table[lowering_idx].clone();
        lowered.substitute(Input { projection, expr });

        *projection = stmt::Projection::single(self.model.lowering.columns[lowering_idx]);

        *expr = lowered;
    }
}

use stmt::ValueStream;

use super::*;

use std::collections::hash_map::Entry;

/// Process the scope component of an insert statement.
struct ApplyInsertScope<'a, 'stmt> {
    expr: &'a mut stmt::Expr<'stmt>,
}

impl<'stmt> Planner<'_, 'stmt> {
    pub(super) fn plan_insert(&mut self, mut stmt: stmt::Insert<'stmt>) -> Option<plan::VarId> {
        self.simplify_stmt_insert(&mut stmt);

        let model = self.model(stmt.target.as_model_id());

        if let stmt::ExprSet::Values(values) = &*stmt.source.body {
            assert!(!values.is_empty(), "stmt={stmt:#?}");
        }

        // If the statement `Returning` is constant (i.e. does not depend on the
        // database evaluating the statement), then extract it here.
        let const_returning = self.extract_const_returning(&mut stmt);

        let filter = match &stmt.target {
            stmt::InsertTarget::Scope(query) => Some(&query.body.as_select().filter),
            _ => None,
        };

        let mut output_var = None;

        // First, lower the returning part of the statement
        let lowered_returning = stmt
            .returning
            .as_mut()
            .map(|returning| self.lower_returning(model, returning));

        let action = match self.insertions.entry(model.id) {
            Entry::Occupied(e) => {
                // TODO
                assert!(!matches!(stmt.returning, Some(stmt::Returning::Expr(_))));
                assert_eq!(
                    self.write_actions[e.get().action]
                        .as_insert()
                        .stmt
                        .returning,
                    stmt.returning
                );
                e.get().action
            }
            Entry::Vacant(e) => {
                // TODO: don't always return values if none are needed.
                let action = self.write_actions.len();

                let mut plan = plan::Insert {
                    input: vec![],
                    output: None,
                    stmt: stmt::Insert {
                        target: stmt::InsertTable {
                            table: model.lowering.table,
                            columns: model.lowering.columns.clone(),
                        }
                        .into(),
                        source: stmt::Values::default().into(),
                        returning: None,
                    },
                };

                if let Some(lowered_returning) = lowered_returning {
                    let var = self.var_table.register_var();
                    plan.output = Some(plan::InsertOutput {
                        var,
                        project: lowered_returning.project,
                    });
                    plan.stmt.returning = stmt.returning.take();

                    output_var = Some(var);
                }

                e.insert(Insertion { action });
                self.push_write_action(plan);

                action
            }
        };

        let rows = match *stmt.source.body {
            stmt::ExprSet::Values(values) => values.rows,
            _ => todo!("stmt={:#?}", stmt),
        };

        for mut row in rows {
            if let Some(filter) = filter {
                self.apply_insert_scope(&mut row, filter);
            }

            self.plan_insert_record(model, row, action);
        }

        output_var
    }

    fn plan_insert_record(&mut self, model: &Model, mut expr: stmt::Expr<'stmt>, action: usize) {
        self.apply_insertion_defaults(model, &mut expr);
        self.plan_insert_relation_stmts(model, &mut expr);
        self.verify_non_nullable_fields_have_values(model, &mut expr);

        self.lower_insert_expr(model, &mut expr);

        self.write_actions[action]
            .as_insert_mut()
            .stmt
            .source
            .body
            .as_values_mut()
            .rows
            .push(expr);
    }

    // Checks all fields of a record and handles nulls
    fn apply_insertion_defaults(&mut self, model: &Model, expr: &mut stmt::Expr<'stmt>) {
        // TODO: make this smarter.. a lot smarter

        // First, we pad the record to account for all fields
        if let stmt::Expr::Record(expr_record) = expr {
            expr_record.resize(model.fields.len(), stmt::Value::Null);
        }

        // Next, we have to find all belongs-to fields and normalize them to FK
        // values
        for field in &model.fields {
            if let FieldTy::BelongsTo(rel) = &field.ty {
                let field_expr = expr.resolve_mut(field.id);

                if !field_expr.is_value() || field_expr.is_null() {
                    continue;
                }

                // Values should be remapped...
                match &rel.foreign_key.fields[..] {
                    [fk_field] => {
                        let e = field_expr.take();
                        expr[fk_field.source.index] = e;
                    }
                    _ => todo!(),
                }
            }
        }

        // We have to handle auto fields first because they are often the
        // identifier which may be referenced to handle associations.
        for field in &model.fields {
            let field_expr = expr.resolve_mut(field.id);

            if field_expr.is_null() {
                // If the field is defined to be auto-populated, then populate
                // it here.
                if let Some(auto) = &field.auto {
                    match auto {
                        Auto::Id => {
                            let id = uuid::Uuid::new_v4().to_string();
                            *field_expr = stmt::Id::from_string(model.id, id).into();
                        }
                    }
                }
            }
        }
    }

    fn verify_non_nullable_fields_have_values(
        &mut self,
        model: &Model,
        expr: &mut stmt::Expr<'stmt>,
    ) {
        for field in &model.fields {
            if field.nullable {
                continue;
            }

            let field_expr = expr.resolve_mut(field.id);

            if field_expr.is_null() {
                // Relations are handled differently
                if !field.ty.is_relation() {
                    panic!(
                        "Insert missing non-nullable field; model={}; field={}; ty={:#?}; expr={:#?}",
                        model.name.upper_camel_case(),
                        field.name,
                        field.ty,
                        expr
                    );
                }
            }
        }
    }

    fn plan_insert_relation_stmts(&mut self, model: &Model, expr: &mut stmt::Expr<'stmt>) {
        for (i, field) in model.fields.iter().enumerate() {
            if expr[i].is_null() {
                if !field.nullable && field.ty.is_has_one() {
                    panic!(
                        "Insert missing non-nullable field; model={}; field={}; ty={:#?}; expr={:#?}",
                        model.name.upper_camel_case(),
                        field.name,
                        field.ty,
                        expr
                    );
                }

                continue;
            }

            if let Some(has_many) = field.ty.as_has_many() {
                // For now, we need to keep this separate
                assert!(!self.insertions.contains_key(&has_many.target));

                let scope = self.inserted_query_stmt(model, expr);
                self.plan_mut_has_many_expr(has_many, expr[i].take(), &scope);
            } else if let Some(has_one) = field.ty.as_has_one() {
                // For now, we need to keep this separate
                assert!(!self.insertions.contains_key(&has_one.target));

                /*
                let scope = self.inserted_query_stmt(model, expr);
                self.plan_mut_has_one_expr(has_one, expr[i].take(), &scope, true);
                */
                todo!()
            } else if let Some(belongs_to) = field.ty.as_belongs_to() {
                if expr[i].is_stmt() {
                    let expr_stmt = expr[i].take().into_stmt();
                    let scope = self.inserted_query_stmt(model, expr);

                    self.plan_mut_belongs_to_stmt(
                        field,
                        *expr_stmt.stmt,
                        &mut expr[i],
                        &scope,
                        true,
                    );

                    match expr[i].take() {
                        stmt::Expr::Value(value) => match value {
                            stmt::Value::Null => {}
                            stmt::Value::Record(_) => todo!("composite key"),
                            value => {
                                let [fk_field] = &belongs_to.foreign_key.fields[..] else {
                                    todo!()
                                };
                                expr[fk_field.source.index] = value.into();
                            }
                        },
                        e => todo!("expr={:#?}", e),
                    }
                }
            }
        }
    }

    /// Returns a select statement that will select the newly inserted record
    fn inserted_query_stmt(&self, model: &Model, expr: &stmt::Expr<'stmt>) -> stmt::Query<'stmt> {
        // The owner's primary key
        let mut args = vec![];

        for pk_field in model.primary_key_fields() {
            let expr = eval::Expr::from_stmt(expr[pk_field.id.index].clone());
            args.push(expr.eval_const());
        }

        model.find_by_id(self.schema, stmt::substitute::Args(&args[..]))
    }

    fn apply_insert_scope(&mut self, expr: &mut stmt::Expr<'stmt>, scope: &stmt::Expr<'stmt>) {
        ApplyInsertScope { expr }.apply(scope);
    }

    fn extract_const_returning(
        &self,
        stmt: &mut stmt::Insert<'stmt>,
    ) -> Option<ValueStream<'stmt>> {
        if matches!(stmt.returning, Some(stmt::Returning::Expr(_))) {
            todo!("stmt={stmt:#?}");
        }

        None
    }
}

impl<'a, 'stmt> ApplyInsertScope<'a, 'stmt> {
    fn apply(&mut self, expr: &stmt::Expr<'stmt>) {
        self.apply_expr(expr, true);
    }

    fn apply_expr(&mut self, stmt: &stmt::Expr<'stmt>, set: bool) {
        match stmt {
            stmt::Expr::And(exprs) => {
                for expr in exprs {
                    self.apply_expr(expr, set);
                }
            }
            stmt::Expr::BinaryOp(e) if e.op.is_eq() => match (&*e.lhs, &*e.rhs) {
                (stmt::Expr::Project(lhs), stmt::Expr::Value(rhs)) => {
                    self.apply_eq_const(&lhs.projection, rhs, set);
                }
                (stmt::Expr::Value(lhs), stmt::Expr::Project(rhs)) => {
                    self.apply_eq_const(&rhs.projection, lhs, set);
                }
                _ => todo!(),
            },
            // Constants are ignored
            stmt::Expr::Value(_) => {}
            _ => todo!("EXPR = {:#?}", stmt),
        }
    }

    fn apply_eq_const(
        &mut self,
        projection: &stmt::Projection,
        val: &stmt::Value<'stmt>,
        set: bool,
    ) {
        let existing = self.expr.resolve_mut(projection);

        if !existing.is_null() {
            if let stmt::Expr::Value(existing) = existing {
                assert_eq!(existing, val);
            } else {
                todo!()
            }
        } else if set {
            *existing = val.clone().into();
        } else {
            todo!()
        }
    }
}

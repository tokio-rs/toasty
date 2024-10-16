use super::*;

use std::collections::hash_map::Entry;

/// Process the scope component of an insert statement.
struct ApplyInsertScope<'a, 'stmt> {
    expr: &'a mut stmt::Expr<'stmt>,
}

impl<'stmt> Planner<'stmt> {
    pub(super) fn plan_insert(&mut self, mut stmt: stmt::Insert<'stmt>) -> Option<plan::VarId> {
        self.simplify_stmt_insert(&mut stmt);
        println!("plan_insert(simplified) = {:#?}", stmt);

        let model = self.model(stmt.scope.body.as_select().source.as_model_id());

        if let stmt::Expr::Record(record) = &stmt.values {
            assert!(!record.is_empty());
        }

        let filter = &stmt.scope.body.as_select().filter;

        let action = match self.insertions.entry(model.id) {
            Entry::Occupied(e) => {
                // Lol, clean this up
                if let Some(returning) = &stmt.returning {
                    let stmt::Returning::Expr(expr) = returning else {
                        todo!("handle other returning")
                    };

                    match &model.primary_key.fields[..] {
                        [pk_field] => {
                            let stmt::Expr::Project(expr_project) = expr else {
                                todo!()
                            };
                            let [step] = expr_project.projection.as_slice() else {
                                todo!()
                            };
                            assert_eq!(step.into_usize(), pk_field.index);
                        }
                        _ => todo!(),
                    }
                }

                e.get().action
            }
            Entry::Vacant(e) => {
                let action = self.write_actions.len();
                let mut returning = vec![];

                e.insert(Insertion { action });

                for column in &model.lowering.columns {
                    returning.push(sql::Expr::column(column));
                }

                self.push_write_action(plan::Insert {
                    input: vec![],
                    output: None,
                    stmt: sql::Insert {
                        table: model.lowering.table,
                        columns: model.lowering.columns.clone(),
                        source: Box::new(sql::Query::values(sql::Values::default())),
                        returning: Some(returning),
                    },
                });

                action
            }
        };

        // This entire thing is bogus
        let mut returning_pk = if let Some(stmt::Returning::Expr(e)) = &stmt.returning {
            match e {
                stmt::Expr::Project(expr_project) => {
                    let [step] = &expr_project.projection[..] else {
                        todo!()
                    };
                    let [pk] = &model.primary_key.fields[..] else {
                        todo!()
                    };

                    if step.into_usize() == pk.index {
                        Some(vec![])
                    } else {
                        todo!()
                    }
                }
                _ => todo!(),
            }
        } else {
            None
        };

        let records = match stmt.values {
            stmt::Expr::Record(records) => records,
            _ => todo!("stmt={:#?}", stmt),
        };

        for mut entry in records {
            if !filter.is_true() {
                self.apply_insert_scope(&mut entry, filter);
            }

            self.plan_insert_record(model, entry, action, returning_pk.as_mut());
        }

        let output_var;
        let output_plan;

        match &stmt.returning {
            Some(stmt::Returning::Star) => {
                let project = eval::Expr::from_stmt(model.lowering.table_to_model.clone().into());

                // TODO: cache this
                let ty = stmt::Type::Record(
                    model
                        .lowering
                        .columns
                        .iter()
                        .map(|column_id| self.schema.column(column_id).ty.clone())
                        .collect(),
                );

                let var = self.var_table.register_var();
                output_plan = Some(plan::InsertOutput { var, project, ty });
                output_var = Some(var);
            }
            Some(stmt::Returning::Expr(_)) => {
                // TODO: this isn't actually correct
                output_var = Some(self.set_var(returning_pk.unwrap()));
                output_plan = None;
            }
            None => {
                output_var = None;
                output_plan = None;
            }
        };

        let action = self.insertions[&model.id].action;
        self.write_actions[action].as_insert_mut().output = output_plan;

        output_var
    }

    fn plan_insert_record(
        &mut self,
        model: &Model,
        mut expr: stmt::Expr<'stmt>,
        action: usize,
        returning_pk: Option<&mut Vec<stmt::Value<'stmt>>>,
    ) {
        self.apply_insertion_defaults(model, &mut expr);
        self.plan_insert_relation_stmts(model, &mut expr);
        self.verify_non_nullable_fields_have_values(model, &mut expr);
        self.apply_fk_association(model, &expr, returning_pk);

        let lowered = self.lower_insert_expr(model, expr);

        self.write_actions[action]
            .as_insert_mut()
            .stmt
            .source
            .as_values_mut()
            .rows
            .push(lowered);
    }

    // Checks all fields of a record and handles nulls
    fn apply_insertion_defaults(&mut self, model: &Model, expr: &mut stmt::Expr<'stmt>) {
        // TODO: make this smarter.. a lot smarter

        // First, we have to find all belongs-to fields and normalize them to FK values
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

                let scope = self.inserted_query_stmt(model, expr);
                self.plan_mut_has_one_expr(has_one, expr[i].take(), &scope, true);
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

    fn apply_fk_association(
        &self,
        model: &Model,
        expr: &stmt::Expr<'stmt>,
        returning_pk: Option<&mut Vec<stmt::Value<'stmt>>>,
    ) {
        if let Some(keys) = returning_pk {
            let mut pk = model.primary_key_fields();

            if pk.len() == 1 {
                let i = pk.next().unwrap().id.index;
                // TODO: clean this up
                keys.push(eval::Expr::from_stmt(expr[i].clone()).eval_const());
            } else {
                todo!("TODO: batch insert relations with composite PK");
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

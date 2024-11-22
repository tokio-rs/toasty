use stmt::ValueStream;

use super::*;

use std::collections::hash_map::Entry;

/// Process the scope component of an insert statement.
struct ApplyInsertScope<'a> {
    expr: &'a mut stmt::Expr,
}

impl Planner<'_> {
    pub(super) fn plan_insert(&mut self, mut stmt: stmt::Insert) -> Option<plan::VarId> {
        self.simplify_stmt_insert(&mut stmt);

        let model = self.model(stmt.target.as_model_id());

        if let stmt::ExprSet::Values(values) = &*stmt.source.body {
            assert!(!values.is_empty(), "stmt={stmt:#?}");
        }

        // Do initial pre-processing of insertion values (apply defaults, apply
        // scope, check constraints, ...)
        self.preprocess_insert_values(model, &mut stmt);

        // If the statement `Returning` is constant (i.e. does not depend on the
        // database evaluating the statement), then extract it here.
        let const_returning = self.constantize_insert_returning(&mut stmt);

        let mut output_var = None;

        // First, lower the returning part of the statement and get any
        // necessary in-memory projection.
        let project = stmt.returning.as_mut().map(|returning| {
            self.lower_returning(model, returning);
            self.partition_returning(returning)
        });

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

                if let Some(project) = project {
                    let var = self.var_table.register_var();
                    plan.output = Some(plan::InsertOutput { var, project });
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
            self.plan_insert_record(model, row, action);
        }

        if let Some(const_returning) = const_returning {
            assert!(output_var.is_none());

            output_var = Some(self.set_var(const_returning));
        }

        output_var
    }

    fn preprocess_insert_values(&mut self, model: &Model, stmt: &mut stmt::Insert) {
        let stmt::ExprSet::Values(values) = &mut *stmt.source.body else {
            todo!()
        };

        let scope_expr = match &stmt.target {
            stmt::InsertTarget::Scope(query) => Some(&query.body.as_select().filter),
            _ => None,
        };

        for row in &mut values.rows {
            if let Some(scope_expr) = scope_expr {
                self.apply_insert_scope(row, scope_expr);
            }

            self.apply_insertion_defaults(model, row);
        }
    }

    fn plan_insert_record(&mut self, model: &Model, mut expr: stmt::Expr, action: usize) {
        self.plan_insert_relation_stmts(model, &mut expr);
        // TODO: move this to pre-processing step, but it currently depends on
        // planning relation statements which cannot be in preprocessing.
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
    fn apply_insertion_defaults(&mut self, model: &Model, expr: &mut stmt::Expr) {
        // TODO: make this smarter.. a lot smarter

        // First, we pad the record to account for all fields
        if let stmt::Expr::Record(expr_record) = expr {
            expr_record.resize(model.fields.len(), stmt::Value::Null);
        }

        // Next, we have to find all belongs-to fields and normalize them to FK
        // values
        for field in &model.fields {
            if let FieldTy::BelongsTo(rel) = &field.ty {
                let field_expr = &mut expr[field.id.index];

                if !field_expr.is_value() || field_expr.is_value_null() {
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
            let field_expr = &mut expr[field.id.index];

            if field_expr.is_value_null() {
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

    fn verify_non_nullable_fields_have_values(&mut self, model: &Model, expr: &mut stmt::Expr) {
        for field in &model.fields {
            if field.nullable {
                continue;
            }

            let field_expr = &expr[field.id.index];

            if field_expr.is_value_null() {
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

    fn plan_insert_relation_stmts(&mut self, model: &Model, expr: &mut stmt::Expr) {
        for (i, field) in model.fields.iter().enumerate() {
            if expr[i].is_value_null() {
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

    /// Returns a select statement that will select the newly inserted record
    fn inserted_query_stmt(&self, model: &Model, expr: &stmt::Expr) -> stmt::Query {
        // The owner's primary key
        let mut args = vec![];

        for pk_field in model.primary_key_fields() {
            let expr = eval::Expr::from(expr[pk_field.id.index].clone());
            args.push(expr.eval_const());
        }

        model.find_by_id(self.schema, stmt::substitute::Args(&args[..]))
    }

    fn apply_insert_scope(&mut self, expr: &mut stmt::Expr, scope: &stmt::Expr) {
        ApplyInsertScope { expr }.apply(scope);
    }

    // TODO: unify with update?
    fn constantize_insert_returning(&self, stmt: &mut stmt::Insert) -> Option<Vec<stmt::Value>> {
        let Some(stmt::Returning::Expr(returning)) = &stmt.returning else {
            return None;
        };

        let stmt::ExprSet::Values(values) = &*stmt.source.body else {
            return None;
        };

        struct ConstReturning;

        impl eval::Convert for ConstReturning {
            fn convert_expr_field(&mut self, field: stmt::ExprField) -> Option<eval::Expr> {
                Some(eval::Expr::arg_project(0, [field.field.index]))
            }
        }

        let returning = eval::Expr::convert_stmt(returning.clone(), ConstReturning).unwrap();

        let mut rows = vec![];

        // TODO: OPTIMIZE!
        for row in &values.rows {
            let evaled = returning.eval([row]).unwrap();
            rows.push(evaled);
        }

        // The returning portion of the statement has been extracted as a const.
        // We do not need to receive it from the database anymore.
        stmt.returning = None;

        Some(rows)
    }
}

impl ApplyInsertScope<'_> {
    fn apply(&mut self, expr: &stmt::Expr) {
        self.apply_expr(expr, true);
    }

    fn apply_expr(&mut self, stmt: &stmt::Expr, set: bool) {
        match stmt {
            stmt::Expr::And(exprs) => {
                for expr in exprs {
                    self.apply_expr(expr, set);
                }
            }
            stmt::Expr::BinaryOp(e) if e.op.is_eq() => match (&*e.lhs, &*e.rhs) {
                (stmt::Expr::Field(lhs), stmt::Expr::Value(rhs)) => {
                    self.apply_eq_const(lhs.field, rhs, set);
                }
                (stmt::Expr::Value(lhs), stmt::Expr::Field(rhs)) => {
                    self.apply_eq_const(rhs.field, lhs, set);
                }
                _ => todo!(),
            },
            // Constants are ignored
            stmt::Expr::Value(_) => {}
            _ => todo!("EXPR = {:#?}", stmt),
        }
    }

    fn apply_eq_const(&mut self, field: FieldId, val: &stmt::Value, set: bool) {
        let existing = &mut self.expr[field];

        if !existing.is_value_null() {
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

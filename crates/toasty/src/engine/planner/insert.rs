use super::*;

use crate::Result;
use db::ColumnId;

use std::collections::hash_map::Entry;

/// Process the scope component of an insert statement.
struct ApplyInsertScope<'a> {
    expr: &'a mut stmt::Expr,
}

impl Planner<'_> {
    pub(super) fn plan_stmt_insert(
        &mut self,
        mut stmt: stmt::Insert,
    ) -> Result<Option<plan::VarId>> {
        let model = self.model(stmt.target.as_model());

        if let stmt::ExprSet::Values(values) = &stmt.source.body {
            assert!(!values.is_empty(), "stmt={stmt:#?}");
        }

        // Do initial pre-processing of insertion values (apply defaults, apply
        // scope, check constraints, ...)
        self.preprocess_insert_values(model, &mut stmt)?;

        self.lower_stmt_insert(model, &mut stmt);

        // If the statement `Returning` is constant (i.e. does not depend on the
        // database evaluating the statement), then extract it here.
        let const_returning = self.constantize_insert_returning(&mut stmt);

        let mut output_var = None;

        // First, lower the returning part of the statement and get any
        // necessary in-memory projection.
        let project = stmt
            .returning
            .as_mut()
            .map(|returning| self.partition_returning(returning));

        let action = match self.insertions.entry(model.id) {
            Entry::Occupied(e) => {
                let existing = &self.write_actions[e.get().action]
                    .as_insert()
                    .stmt
                    .returning;

                // TODO
                match stmt.returning {
                    Some(stmt::Returning::Star) => {
                        assert!(matches!(existing, Some(stmt::Returning::Star)));
                    }
                    None => {
                        assert!(existing.is_none());
                    }
                    _ => todo!(),
                }

                e.get().action
            }
            Entry::Vacant(e) => {
                // TODO: don't always return values if none are needed.
                let action = self.write_actions.len();

                let mut plan = plan::Insert {
                    input: None,
                    output: None,
                    stmt: stmt::Insert {
                        target: stmt.target.clone(),
                        source: stmt::Values::default().into(),
                        returning: stmt.returning.take(),
                    },
                };

                if let Some(project) = project {
                    let var = self.var_table.register_var(project.ret.clone());
                    plan.output = Some(plan::Output { var, project });
                    output_var = Some(var);
                }

                e.insert(Insertion { action });
                self.push_write_action(plan);

                action
            }
        };

        let rows = match stmt.source.body {
            stmt::ExprSet::Values(values) => values.rows,
            _ => todo!("stmt={:#?}", stmt),
        };

        let dst = &mut self.write_actions[action]
            .as_insert_mut()
            .stmt
            .source
            .body
            .as_values_mut()
            .rows;

        dst.extend(rows);

        if let Some((values, ty)) = const_returning {
            assert!(output_var.is_none());

            output_var = Some(self.set_var(values, ty));
        }

        Ok(output_var)
    }

    fn preprocess_insert_values(
        &mut self,
        model: &app::Model,
        stmt: &mut stmt::Insert,
    ) -> Result<()> {
        let stmt::ExprSet::Values(values) = &mut stmt.source.body else {
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
            self.plan_insert_relation_stmts(model, row)?;
            self.verify_field_constraints(model, row)?;
        }

        Ok(())
    }

    // Checks all fields of a record and handles nulls
    fn apply_insertion_defaults(&mut self, model: &app::Model, expr: &mut stmt::Expr) {
        // TODO: make this smarter.. a lot smarter

        // First, we pad the record to account for all fields
        if let stmt::Expr::Record(expr_record) = expr {
            // TODO: get rid of this
            assert_eq!(expr_record.len(), model.fields.len());
            // expr_record.resize(model.fields.len(), stmt::Value::Null);
        }

        // Next, we have to find all belongs-to fields and normalize them to FK
        // values
        for field in &model.fields {
            if let app::FieldTy::BelongsTo(rel) = &field.ty {
                let [fk_field] = &rel.foreign_key.fields[..] else {
                    todo!()
                };

                let mut field_expr = expr.entry_mut(field.id.index);

                if !field_expr.is_value() || field_expr.is_value_null() {
                    continue;
                }

                let e = field_expr.take();
                expr.entry_mut(fk_field.source.index).insert(e);
            }
        }

        // We have to handle auto fields first because they are often the
        // identifier which may be referenced to handle associations.
        for field in &model.fields {
            let mut field_expr = expr.entry_mut(field.id.index);

            if field_expr.is_value_null() {
                // If the field is defined to be auto-populated, then populate
                // it here.
                if let Some(auto) = &field.auto {
                    match auto {
                        app::Auto::Id => {
                            let id = uuid::Uuid::new_v4().to_string();
                            field_expr.insert(stmt::Id::from_string(model.id, id).into());
                        }
                    }
                }
            }
        }
    }

    fn verify_field_constraints(
        &mut self,
        model: &app::Model,
        expr: &mut stmt::Expr,
    ) -> Result<()> {
        for field in &model.fields {
            if field.nullable && field.constraints.is_empty() {
                continue;
            }

            let field_expr = expr.entry(field.id.index);

            if !field.nullable && field_expr.is_value_null() {
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

            for constraint in &field.constraints {
                constraint.check(&field_expr)?;
            }
        }

        Ok(())
    }

    fn plan_insert_relation_stmts(
        &mut self,
        model: &app::Model,
        expr: &mut stmt::Expr,
    ) -> Result<()> {
        for (i, field) in model.fields.iter().enumerate() {
            if expr.entry(i).is_value_null() {
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
                self.plan_mut_has_many_expr(
                    has_many,
                    stmt::AssignmentOp::Insert,
                    expr.entry_mut(i).take(),
                    &scope,
                )?;
            } else if let Some(has_one) = field.ty.as_has_one() {
                // For now, we need to keep this separate
                assert!(!self.insertions.contains_key(&has_one.target));

                let scope = self.inserted_query_stmt(model, expr);
                self.plan_mut_has_one_expr(has_one, expr.entry_mut(i).take(), &scope, true)?;
            } else if let Some(belongs_to) = field.ty.as_belongs_to() {
                let mut entry = expr.entry_mut(i);

                if entry.is_statement() {
                    let expr_stmt = entry.take().into_stmt();
                    let scope = self.inserted_query_stmt(model, expr);
                    let mut entry = expr.entry_mut(i);

                    debug_assert!(entry.is_expr(), "entry={entry:#?}");

                    self.plan_mut_belongs_to_stmt(
                        field,
                        *expr_stmt.stmt,
                        entry.as_expr_mut(),
                        &scope,
                        true,
                    )?;

                    match entry.take() {
                        stmt::Expr::Value(value) => match value {
                            stmt::Value::Null => {}
                            stmt::Value::Record(_) => todo!("composite key"),
                            value => {
                                let [fk_field] = &belongs_to.foreign_key.fields[..] else {
                                    todo!()
                                };
                                expr.entry_mut(fk_field.source.index).insert(value.into());
                            }
                        },
                        e => todo!("expr={:#?}", e),
                    }
                }
            }
        }

        Ok(())
    }

    /// Returns a select statement that will select the newly inserted record
    fn inserted_query_stmt(&self, model: &app::Model, expr: &stmt::Expr) -> stmt::Query {
        // The owner's primary key
        let mut args = vec![];

        for pk_field in model.primary_key_fields() {
            // let expr = eval::Expr::from(expr.entry(pk_field.id.index).to_value());
            args.push(expr.entry(pk_field.id.index).to_value());
        }

        model.find_by_id(&args)
    }

    fn apply_insert_scope(&mut self, expr: &mut stmt::Expr, scope: &stmt::Expr) {
        ApplyInsertScope { expr }.apply(scope);
    }

    // TODO: unify with update?
    fn constantize_insert_returning(
        &self,
        stmt: &mut stmt::Insert,
    ) -> Option<(Vec<stmt::Value>, stmt::Type)> {
        let Some(stmt::Returning::Expr(returning)) = &stmt.returning else {
            return None;
        };

        let stmt::ExprSet::Values(values) = &stmt.source.body else {
            todo!("stmt={stmt:#?}");
        };

        let stmt::InsertTarget::Table(insert_table) = &stmt.target else {
            todo!("stmt={stmt:#?}");
        };

        struct ConstReturning<'a> {
            columns: &'a [ColumnId],
        }

        impl eval::Convert for ConstReturning<'_> {
            fn convert_expr_column(&mut self, stmt: &stmt::ExprColumn) -> Option<stmt::Expr> {
                let index = self
                    .columns
                    .iter()
                    .position(|column| stmt.references(*column))
                    .unwrap();

                Some(stmt::Expr::arg_project(0, [index]))
            }
        }

        let args = stmt::Type::Record(
            insert_table
                .columns
                .iter()
                .map(|column_id| self.schema.db.column(*column_id).ty.clone())
                .collect(),
        );

        let expr = eval::Func::try_convert_from_stmt(
            returning.clone(),
            vec![args],
            ConstReturning {
                columns: &insert_table.columns,
            },
        )
        .unwrap();

        let mut rows = vec![];

        // TODO: OPTIMIZE!
        for row in &values.rows {
            let evaled = expr.eval([row]).unwrap();
            rows.push(evaled);
        }

        // The returning portion of the statement has been extracted as a const.
        // We do not need to receive it from the database anymore.
        stmt.returning = None;

        Some((rows, stmt::Type::list(expr.ret)))
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

    fn apply_eq_const(&mut self, field: app::FieldId, val: &stmt::Value, set: bool) {
        let mut existing = self.expr.entry_mut(field.index);

        if !existing.is_value_null() {
            if let stmt::EntryMut::Value(existing) = existing {
                assert_eq!(existing, val);
            } else {
                todo!()
            }
        } else if set {
            existing.insert(val.clone().into());
        } else {
            todo!()
        }
    }
}

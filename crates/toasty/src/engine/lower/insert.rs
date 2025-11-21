use toasty_core::{schema::app, stmt};

use crate::engine::lower::LowerStatement;

/// Process the scope component of an insert statement.
struct ApplyInsertScope<'a> {
    expr: &'a mut stmt::Expr,
}

impl LowerStatement<'_, '_> {
    // First, apply the insertion scope to the insertion values
    pub(super) fn apply_insert_scope(
        &mut self,
        target: &mut stmt::InsertTarget,
        source: &mut stmt::Query,
    ) {
        let stmt::InsertTarget::Scope(scope) = target else {
            // Insertion is not targetting a scope
            return;
        };

        let stmt::ExprSet::Values(values) = &mut source.body else {
            todo!()
        };

        let scope = &scope.body.as_select_unwrap();

        if let Some(filter) = &scope.filter.expr {
            for expr in &mut values.rows {
                ApplyInsertScope { expr }.apply_expr(filter)
            }
        }

        *target = stmt::InsertTarget::Model(scope.source.model_id_unwrap());
    }

    pub(super) fn preprocess_insert_values(&mut self, source: &mut stmt::Query) {
        let stmt::ExprSet::Values(values) = &mut source.body else {
            todo!()
        };

        let Some(model) = self.expr_cx.target_as_model() else {
            return;
        };

        for row in &mut values.rows {
            self.apply_app_level_insertion_defaults(model, row);
            self.plan_stmt_insert_relations(row);
            self.verify_field_constraints(model, row);
        }
    }

    // Checks all fields of a record and handles nulls
    fn apply_app_level_insertion_defaults(&mut self, model: &app::Model, expr: &mut stmt::Expr) {
        // First, we pad the record to account for all fields
        if let stmt::Expr::Record(expr_record) = expr {
            // TODO: get rid of this
            assert_eq!(expr_record.len(), model.fields.len());
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

    fn verify_field_constraints(&mut self, model: &app::Model, expr: &mut stmt::Expr) {
        for field in &model.fields {
            if field.nullable && field.constraints.is_empty() {
                continue;
            }

            let field_expr = expr.entry(field.id.index);

            if !field.nullable && field_expr.is_value_null() {
                // Relations are handled differently
                if !field.ty.is_relation() {
                    panic!(
                        "Insert missing non-nullable field; model={}; field={:#?}; expr={:#?}",
                        model.name.upper_camel_case(),
                        field,
                        expr
                    );
                }
            }

            for constraint in &field.constraints {
                if let Err(err) = constraint.check(&field_expr) {
                    self.state.errors.push(err);
                }
            }
        }
    }
}

impl ApplyInsertScope<'_> {
    fn apply_expr(&mut self, stmt: &stmt::Expr) {
        match stmt {
            stmt::Expr::And(exprs) => {
                for expr in exprs {
                    self.apply_expr(expr);
                }
            }
            stmt::Expr::BinaryOp(e) if e.op.is_eq() => match (&*e.lhs, &*e.rhs) {
                (
                    stmt::Expr::Reference(expr_ref @ stmt::ExprReference::Field { .. }),
                    stmt::Expr::Value(rhs),
                ) => {
                    self.apply_eq_const(expr_ref, rhs);
                }
                (
                    stmt::Expr::Value(lhs),
                    stmt::Expr::Reference(expr_ref @ stmt::ExprReference::Field { .. }),
                ) => {
                    self.apply_eq_const(expr_ref, lhs);
                }
                _ => todo!(),
            },
            // Constants are ignored
            stmt::Expr::Value(_) => {}
            _ => todo!("EXPR = {:#?}", stmt),
        }
    }

    fn apply_eq_const(&mut self, expr_ref: &stmt::ExprReference, val: &stmt::Value) {
        let stmt::ExprReference::Field { nesting, index } = expr_ref else {
            todo!("handle non-field reference");
        };

        assert!(*nesting == 0, "TODO: handle references to parent scopes");

        let mut existing = self.expr.entry_mut(*index);

        if !existing.is_value_null() && !existing.is_default() {
            if let stmt::EntryMut::Value(existing) = existing {
                assert_eq!(existing, val);
            } else {
                todo!()
            }
        } else {
            existing.insert(val.clone().into());
        }
    }
}

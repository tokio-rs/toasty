use bit_set::BitSet;
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

    pub(super) fn preprocess_insert_values(
        &mut self,
        source: &mut stmt::Query,
        returning: &mut Option<stmt::Returning>,
    ) {
        let stmt::ExprSet::Values(values) = &mut source.body else {
            todo!()
        };

        let Some(model) = self.expr_cx.target_as_model() else {
            return;
        };

        let mut set_fields: BitSet<usize> = BitSet::default();

        // First, apply any defaults while also tracking all the fields that are set.
        for (index, row) in values.rows.iter_mut().enumerate() {
            self.lower_insert_with_row(index, |lower| {
                lower.apply_app_level_insertion_defaults(model, row, &mut set_fields);
            });
        }

        // If there are any has_n associations included in the insertion, the
        // statement returning has to be transformed to accomodate the nested
        // structure.
        self.convert_returning_for_insert(values, returning, source.single);

        for (index, row) in values.rows.iter_mut().enumerate() {
            self.lower_insert_with_row(index, |lower| {
                lower.plan_stmt_insert_relations(row, returning, index);
                lower.verify_field_constraints(model, row);
            });
        }
    }

    // Checks all fields of a record and handles nulls
    fn apply_app_level_insertion_defaults(
        &mut self,
        model: &app::Model,
        expr: &mut stmt::Expr,
        set_fields: &mut BitSet<usize>,
    ) {
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

            if field_expr.is_default() {
                // If the field is defined to be auto-populated, then populate
                // it here.
                if let Some(auto) = &field.auto {
                    let ty = match &field.ty {
                        app::FieldTy::Primitive(primitive) => &primitive.ty,
                        _ => panic!("#[auto] not allowed on non-primitive fields"),
                    };
                    match auto {
                        app::AutoStrategy::Id => {
                            let id = uuid::Uuid::new_v4().to_string();
                            field_expr.insert(stmt::Id::from_string(model.id, id).into());
                        }
                        app::AutoStrategy::Uuid(version) => {
                            let id = match version {
                                app::UuidVersion::V4 => uuid::Uuid::new_v4(),
                                app::UuidVersion::V7 => uuid::Uuid::now_v7(),
                            };
                            match ty {
                                stmt::Type::String => field_expr.insert(stmt::Value::String(id.to_string()).into()),
                                stmt::Type::Uuid => field_expr.insert(stmt::Value::Uuid(id).into()),
                                _ => panic!("auto-generated UUID cannot be inserted into column of type {ty:?}"),
                            };
                        }
                        app::AutoStrategy::Increment => {
                            // Leave value as `Expr::Default` and let the database handle it.
                        }
                    }
                }
            }

            if !field_expr.is_value_null() {
                set_fields.insert(field.id.index);
            }
        }
    }

    fn convert_returning_for_insert(
        &mut self,
        values: &stmt::Values,
        returning: &mut Option<stmt::Returning>,
        single: bool,
    ) {
        // If there is no returning statement, there is nothing to convert
        let Some(stmt::Returning::Expr(projection)) = returning else {
            return;
        };

        #[derive(Debug)]
        struct Input(usize);

        impl stmt::Input for Input {
            fn resolve_arg(
                &mut self,
                expr_arg: &stmt::ExprArg,
                projection: &stmt::Projection,
            ) -> Option<stmt::Expr> {
                todo!("self={self:#?}; expr_arg={expr_arg:#?}; projection={projection:#?}");
            }

            fn resolve_ref(
                &mut self,
                expr_reference: &stmt::ExprReference,
                projection: &stmt::Projection,
            ) -> Option<stmt::Expr> {
                let expr_column = expr_reference.as_expr_column()?;

                assert!(
                    expr_column.nesting == 0 && expr_column.table == 0,
                    "expr_reference={expr_reference:#?}"
                );
                assert!(projection.is_identity(), "TODO");

                Some(stmt::Expr::project(*expr_reference, self.0))
            }
        }

        let mut converted = vec![];

        for i in 0..values.rows.len() {
            let mut converted_row = projection.clone();
            converted_row.substitute(Input(i));
            converted.push(converted_row);
        }

        *returning = Some(stmt::Returning::Value(if single {
            assert!(converted.len() == 1);
            converted.into_iter().next().unwrap()
        } else {
            stmt::Expr::list_from_vec(converted)
        }));
    }

    fn verify_field_constraints(&mut self, model: &app::Model, expr: &mut stmt::Expr) {
        for field in &model.fields {
            if field.nullable && field.constraints.is_empty() {
                continue;
            }

            let field_expr = expr.entry(field.id.index).unwrap();

            if !field.nullable && field_expr.is_value_null() {
                // Relations are handled differently
                if !field.ty.is_relation() && field.auto.is_none() {
                    self.state
                        .errors
                        .push(toasty_core::Error::validation_failed(format!(
                            "insert missing non-nullable field `{}` in model `{}`",
                            field.name.app_name,
                            model.name.upper_camel_case()
                        )));
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
                    rhs @ stmt::Expr::Value(..),
                ) => {
                    self.apply_eq_constraint(expr_ref, rhs);
                }
                (
                    lhs @ stmt::Expr::Value(..),
                    stmt::Expr::Reference(expr_ref @ stmt::ExprReference::Field { .. }),
                ) => {
                    self.apply_eq_constraint(expr_ref, lhs);
                }
                (
                    lhs_expr @ stmt::Expr::Reference(
                        lhs @ stmt::ExprReference::Field {
                            nesting: nesting_lhs,
                            ..
                        },
                    ),
                    rhs_expr @ stmt::Expr::Reference(
                        rhs @ stmt::ExprReference::Field {
                            nesting: nesting_rhs,
                            ..
                        },
                    ),
                ) => match (nesting_lhs, nesting_rhs) {
                    (0, _) if *nesting_rhs > 0 => self.apply_eq_constraint(lhs, rhs_expr),
                    (_, 0) if *nesting_lhs > 0 => self.apply_eq_constraint(rhs, lhs_expr),
                    _ => panic!("exactly one field must reference parent"),
                },
                _ => todo!("EXPR = {:#?}", stmt),
            },
            // Constants are ignored
            stmt::Expr::Value(_) => {}
            _ => todo!("EXPR = {:#?}", stmt),
        }
    }

    fn apply_eq_constraint(&mut self, expr_ref: &stmt::ExprReference, val: &stmt::Expr) {
        let stmt::ExprReference::Field { nesting, index } = expr_ref else {
            todo!("handle non-field reference");
        };

        assert!(*nesting == 0, "TODO: handle references to parent scopes");

        let mut existing = self.expr.entry_mut(*index);

        if !existing.is_value_null() && !existing.is_default() {
            if let stmt::EntryMut::Value(existing) = existing {
                if let stmt::Expr::Value(val) = val {
                    assert_eq!(existing, val);
                } else {
                    todo!()
                }
            } else {
                todo!()
            }
        } else {
            existing.insert(val.clone());
        }
    }
}

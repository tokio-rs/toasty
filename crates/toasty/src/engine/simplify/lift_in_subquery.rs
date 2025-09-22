use super::Simplify;
use toasty_core::{
    schema::app::{BelongsTo, FieldId, FieldTy, HasOne},
    stmt::{self, Visit},
};

struct LiftBelongsTo<'a> {
    simplify: &'a Simplify<'a>,
    belongs_to: &'a BelongsTo,
    // TODO: switch to bit field set
    fk_field_matches: Vec<bool>,
    fail: bool,
    operands: Vec<stmt::Expr>,
}

impl Simplify<'_> {
    pub(crate) fn lift_in_subquery(
        &mut self,
        expr: &stmt::Expr,
        query: &stmt::Query,
    ) -> Option<stmt::Expr> {
        // The expression is a path expression referencing a relation.
        let field = match expr {
            stmt::Expr::Project(_) => {
                todo!()
            }
            stmt::Expr::Reference(expr_reference @ stmt::ExprReference::Field { .. }) => self
                .cx
                .resolve_expr_reference(expr_reference)
                .expect_field(),
            _ => {
                return None;
            }
        };

        // If the field is not a belongs_to relation, abort
        let mut maybe_expr = match &field.ty {
            FieldTy::BelongsTo(belongs_to) => self.lift_belongs_to_in_subquery(belongs_to, query),
            FieldTy::HasOne(has_one) => self.lift_has_one_in_subquery(has_one, query),
            _ => {
                return None;
            }
        };

        if let Some(maybe_expr) = &mut maybe_expr {
            stmt::visit_mut::visit_expr_mut(self, maybe_expr);
        }

        maybe_expr
    }

    /// Optimizes queries by lifting BelongsTo relation constraints out of subqueries when possible.
    ///
    /// This is an app-level optimization that operates on the application schema before
    /// statements are lowered to database-specific representations.
    ///
    /// This method analyzes a subquery that filters a related model and determines if the query
    /// can be rewritten to avoid the subquery by directly comparing foreign key fields.
    ///
    /// For example, transforms:
    /// ```sql
    /// -- Original: subquery filtering related records
    /// user_id IN (SELECT id FROM users WHERE name = 'Alice')
    ///
    /// -- Optimized: direct foreign key comparison
    /// user_id = 'Alice_user_id'
    /// ```
    ///
    /// The optimization works by:
    /// 1. Verifying the subquery targets the same model as the BelongsTo relation
    /// 2. Analyzing the subquery's WHERE clause to find constraints on foreign key fields
    /// 3. If all constraints can be lifted, rewriting them as direct field comparisons
    /// 4. If constraints reference non-foreign-key fields, falling back to an IN subquery
    ///
    /// Returns `None` if the subquery cannot be optimized (wrong target model).
    /// Returns `Some(expr)` containing either:
    /// - Direct field comparison expressions (when optimization succeeds)
    /// - An IN subquery expression (when partial optimization is possible)
    ///
    /// Currently only supports single-field foreign keys; composite keys are not yet implemented.
    fn lift_belongs_to_in_subquery(
        &self,
        belongs_to: &BelongsTo,
        query: &stmt::Query,
    ) -> Option<stmt::Expr> {
        if belongs_to.target != query.body.as_select().source.as_model_id() {
            return None;
        }

        let select = query.body.as_select();

        assert_eq!(
            belongs_to.foreign_key.fields.len(),
            1,
            "TODO: composite keys"
        );

        let mut lift = LiftBelongsTo {
            simplify: &self.scope(&select.source),
            belongs_to,
            fk_field_matches: vec![false; belongs_to.foreign_key.fields.len()],
            operands: vec![],
            fail: false,
        };

        lift.visit(&select.filter);

        if lift.fail {
            let [fk_fields] = &belongs_to.foreign_key.fields[..] else {
                todo!("composite keys")
            };
            let mut subquery = query.clone();

            subquery.body.as_select_mut().returning =
                stmt::Returning::Expr(stmt::Expr::field(fk_fields.target));

            Some(stmt::Expr::in_subquery(
                stmt::Expr::field(fk_fields.source),
                subquery,
            ))
        } else {
            Some(if lift.operands.len() == 1 {
                lift.operands.into_iter().next().unwrap()
            } else {
                stmt::ExprAnd {
                    operands: lift.operands,
                }
                .into()
            })
        }
    }

    /// Rewrite the `HasOne` in subquery expression to reference the foreign key.
    fn lift_has_one_in_subquery(
        &self,
        has_one: &HasOne,
        query: &stmt::Query,
    ) -> Option<stmt::Expr> {
        if has_one.target != query.body.as_select().source.as_model_id() {
            return None;
        }

        let pair = has_one.pair(&self.schema().app);

        let expr = match &pair.foreign_key.fields[..] {
            [fk_field] => stmt::Expr::field(fk_field.target),
            _ => todo!("composite"),
        };

        let mut subquery = query.clone();

        match &mut subquery.body {
            stmt::ExprSet::Select(subquery) => {
                subquery.returning = stmt::Returning::Expr(match &pair.foreign_key.fields[..] {
                    [fk_field] => stmt::Expr::field(fk_field.source),
                    _ => todo!("composite key"),
                });
            }
            _ => todo!(),
        };

        Some(
            stmt::ExprInSubquery {
                expr: Box::new(expr),
                query: Box::new(subquery),
            }
            .into(),
        )
    }
}

impl Visit for LiftBelongsTo<'_> {
    fn visit_expr_binary_op(&mut self, i: &stmt::ExprBinaryOp) {
        match (&*i.lhs, &*i.rhs) {
            (stmt::Expr::Reference(expr_reference), other)
            | (other, stmt::Expr::Reference(expr_reference)) => {
                assert!(i.op.is_eq());

                let field = self
                    .simplify
                    .cx
                    .resolve_expr_reference(expr_reference)
                    .expect_field();

                self.lift_fk_constraint(field.id, other);
            }
            _ => {}
        }
    }
}

impl LiftBelongsTo<'_> {
    fn lift_fk_constraint(&mut self, field: FieldId, expr: &stmt::Expr) {
        for (i, fk_field) in self.belongs_to.foreign_key.fields.iter().enumerate() {
            if fk_field.target == field {
                if self.fk_field_matches[i] {
                    todo!("not handled");
                }

                self.operands.push(stmt::Expr::eq(
                    stmt::Expr::field(fk_field.source),
                    expr.clone(),
                ));
                self.fk_field_matches[i] = true;

                return;
            }
        }

        self.fail = true;
    }
}

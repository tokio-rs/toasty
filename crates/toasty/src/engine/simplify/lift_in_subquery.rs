use super::*;
use app::{BelongsTo, FieldId, FieldTy, HasOne};

use stmt::Visit;

struct LiftBelongsTo<'a> {
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
            stmt::Expr::Field(expr) => self.schema.app.field(expr.field),
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

    fn lift_belongs_to_in_subquery(
        &self,
        belongs_to: &BelongsTo,
        query: &stmt::Query,
    ) -> Option<stmt::Expr> {
        if belongs_to.target != query.body.as_select().source.as_model_id() {
            return None;
        }

        let filter = &query.body.as_select().filter;

        assert_eq!(
            belongs_to.foreign_key.fields.len(),
            1,
            "TODO: composite keys"
        );

        let mut lift = LiftBelongsTo {
            belongs_to,
            fk_field_matches: vec![false; belongs_to.foreign_key.fields.len()],
            operands: vec![],
            fail: false,
        };

        lift.visit(filter);

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

        let pair = has_one.pair(&self.schema.app);

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
            (stmt::Expr::Field(expr_field), other) | (other, stmt::Expr::Field(expr_field)) => {
                assert!(i.op.is_eq());
                self.lift_fk_constraint(expr_field.field, other);
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

                self.operands
                    .push(stmt::Expr::eq(fk_field.source, expr.clone()));
                self.fk_field_matches[i] = true;

                return;
            }
        }

        self.fail = true;
    }
}

use std::cmp::PartialOrd;

use super::Simplify;
use toasty_core::stmt::{self, Expr, VisitMut};
use toasty_core::schema::app::{FieldTy, Model};

impl Simplify<'_> {
    pub(super) fn simplify_expr_eq_operand(&mut self, operand: &mut stmt::Expr) {
        if let stmt::Expr::Reference(expr_reference) = operand {
            match &*expr_reference {
                stmt::ExprReference::Model { nesting } => {
                    let model = self
                        .cx
                        .resolve_expr_reference(expr_reference)
                        .expect_model();

                    let [pk_field] = &model.primary_key.fields[..] else {
                        todo!("handle composite keys");
                    };

                    *operand = stmt::Expr::ref_field(*nesting, pk_field);
                }
                stmt::ExprReference::Field { .. } => {
                    let field = self
                        .cx
                        .resolve_expr_reference(expr_reference)
                        .expect_field();

                    match &field.ty {
                        FieldTy::Primitive(_) => {}
                        FieldTy::Embedded(embedded) => {
                            let target = self.schema().app.model(embedded.target);
                            if matches!(target, Model::EmbeddedEnum(_)) {
                                // EmbeddedEnum fields are stored as a single integer
                                // column, so the field reference is already valid as-is.
                            } else {
                                todo!("embedded struct field in binary op")
                            }
                        }
                        FieldTy::HasMany(_) | FieldTy::HasOne(_) => todo!(),
                        FieldTy::BelongsTo(rel) => {
                            let [fk_field] = &rel.foreign_key.fields[..] else {
                                todo!("handle composite keys");
                            };

                            let stmt::ExprReference::Field { index, .. } = expr_reference else {
                                panic!()
                            };
                            *index = fk_field.source.index;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Recursively walk a binary expression in parallel
    pub(super) fn simplify_expr_binary_op(
        &mut self,
        op: stmt::BinaryOp,
        lhs: &mut stmt::Expr,
        rhs: &mut stmt::Expr,
    ) -> Option<stmt::Expr> {
        if op.is_eq() || op.is_ne() {
            self.simplify_expr_eq_operand(lhs);
            self.simplify_expr_eq_operand(rhs);
        }

        match (&mut *lhs, &mut *rhs) {
            // Self-comparison, e.g.,
            //
            //  - `x = x` → `true`
            //  - `x != x` → `false`
            //
            // Only applied for non-nullable field references.
            (Expr::Reference(lhs), Expr::Reference(rhs))
                if lhs == rhs && (op.is_eq() || op.is_ne()) =>
            {
                if lhs.is_field() {
                    let field = self.cx.resolve_expr_reference(lhs).expect_field();
                    if !field.nullable() {
                        return Some(op.is_eq().into());
                    }
                }
                None
            }
            // Constant folding and null propagation,
            //
            //   - `5 = 5` → `true`
            //   - `1 < 5` → `true`
            //   - `"a" >= "b"` → `false`
            //   - `null <op> x` → `null`
            //   - `x <op> null` → `null`
            (Expr::Value(lhs_val), Expr::Value(rhs_val)) => {
                if lhs_val.is_null() || rhs_val.is_null() {
                    return Some(Expr::null());
                }

                match op {
                    stmt::BinaryOp::Eq => Some((*lhs_val == *rhs_val).into()),
                    stmt::BinaryOp::Ne => Some((*lhs_val != *rhs_val).into()),
                    stmt::BinaryOp::Lt => {
                        PartialOrd::partial_cmp(&*lhs_val, &*rhs_val).map(|o| o.is_lt().into())
                    }
                    stmt::BinaryOp::Le => {
                        PartialOrd::partial_cmp(&*lhs_val, &*rhs_val).map(|o| o.is_le().into())
                    }
                    stmt::BinaryOp::Gt => {
                        PartialOrd::partial_cmp(&*lhs_val, &*rhs_val).map(|o| o.is_gt().into())
                    }
                    stmt::BinaryOp::Ge => {
                        PartialOrd::partial_cmp(&*lhs_val, &*rhs_val).map(|o| o.is_ge().into())
                    }
                }
            }
            // Boolean constant comparisons:
            //
            //  - `x = true` → `x`
            //  - `x = false` → `not(x)`
            //  - `x != true` → `not(x)`
            //  - `x != false` → `x`
            (expr, Expr::Value(stmt::Value::Bool(b)))
            | (Expr::Value(stmt::Value::Bool(b)), expr)
                if op.is_eq() || op.is_ne() =>
            {
                let is_eq_true = (op.is_eq() && *b) || (op.is_ne() && !*b);
                if is_eq_true {
                    Some(expr.take())
                } else {
                    Some(Expr::not(expr.take()))
                }
            }
            // Tuple decomposition,
            //
            //  - `(a, b) = (x, y)` → `a = x and b = y`
            //  - `(a, b) != (x, y)` → `a != x or b != y`
            (Expr::Record(lhs_rec), Expr::Record(rhs_rec))
                if (op.is_eq() || op.is_ne()) && lhs_rec.len() == rhs_rec.len() =>
            {
                let comparisons: Vec<_> = std::mem::take(&mut lhs_rec.fields)
                    .into_iter()
                    .zip(std::mem::take(&mut rhs_rec.fields))
                    .map(|(l, r)| Expr::binary_op(l, op, r))
                    .collect();

                if op.is_eq() {
                    Some(Expr::and_from_vec(comparisons))
                } else {
                    Some(Expr::or_from_vec(comparisons))
                }
            }
            // Tuple decomposition with a Value::Record on one side,
            //
            //  - `(a, b) = Value::Record([x, y])` → `a = x and b = y`
            //
            // This arises after match elimination produces `Record([col1, col2]) == Value::Record([1, "alice"])`.
            (Expr::Record(rec), Expr::Value(stmt::Value::Record(val_rec)))
            | (Expr::Value(stmt::Value::Record(val_rec)), Expr::Record(rec))
                if (op.is_eq() || op.is_ne()) && rec.len() == val_rec.len() =>
            {
                let comparisons: Vec<_> = std::mem::take(&mut rec.fields)
                    .into_iter()
                    .zip(std::mem::take(&mut val_rec.fields))
                    .map(|(expr, val)| Expr::binary_op(expr, op, Expr::from(val)))
                    .collect();

                if op.is_eq() {
                    Some(Expr::and_from_vec(comparisons))
                } else {
                    Some(Expr::or_from_vec(comparisons))
                }
            }
            // Match elimination: distribute binary op into match arms as OR
            //
            //   Match(subj, [p1 => e1, p2 => e2]) <op> rhs
            //   → OR(subj == p1 AND e1 <op> rhs, subj == p2 AND e2 <op> rhs)
            //
            // Each arm is fully simplified inline. Arms that fold to false/null
            // are pruned.
            (Expr::Match(_), _) => {
                let match_expr = lhs.take();
                let other = rhs.take();
                Some(self.eliminate_match_in_binary_op(op, match_expr, other, true))
            }
            (_, Expr::Match(_)) => {
                let other = lhs.take();
                let match_expr = rhs.take();
                Some(self.eliminate_match_in_binary_op(op, match_expr, other, false))
            }
            // Canonicalization, `literal <op> col` → `col <op_commuted> literal`
            (Expr::Value(_), rhs) if !rhs.is_value() => {
                std::mem::swap(lhs, rhs);
                Some(Expr::binary_op(lhs.take(), op.commute(), rhs.take()))
            }
            // Self-comparison with projections, e.g.,
            //
            //  - `address.city = address.city` → `true`
            //  - `address.city != address.city` → `false`
            //
            // By this point, constant projections and record projections have been simplified.
            // What remains are projections with opaque bases (e.g., field references).
            (Expr::Project(lhs), Expr::Project(rhs))
                if lhs == rhs && (op.is_eq() || op.is_ne()) =>
            {
                // TODO: Check if the projected value is nullable
                Some(Expr::from(op.is_eq()))
            }
            _ => None,
        }
    }

    /// Distributes a binary op over match arms, producing an OR of guarded
    /// comparisons. Each arm becomes `(subject == pattern) AND (arm_expr <op> other)`.
    /// Dead branches (false/null) are pruned after inline simplification.
    fn eliminate_match_in_binary_op(
        &mut self,
        op: stmt::BinaryOp,
        match_expr: Expr,
        other: Expr,
        match_on_lhs: bool,
    ) -> Expr {
        let Expr::Match(match_expr) = match_expr else {
            unreachable!()
        };

        let mut operands = Vec::new();

        for arm in match_expr.arms {
            let guard = Expr::binary_op(
                (*match_expr.subject).clone(),
                stmt::BinaryOp::Eq,
                Expr::from(arm.pattern),
            );

            let comparison = if match_on_lhs {
                Expr::binary_op(arm.expr, op, other.clone())
            } else {
                Expr::binary_op(other.clone(), op, arm.expr)
            };

            let mut term = Expr::and_from_vec(vec![guard, comparison]);
            self.visit_expr_mut(&mut term);

            // Prune dead branches
            if term.is_false() || matches!(&term, Expr::Value(stmt::Value::Null)) {
                continue;
            }

            operands.push(term);
        }

        // The else arm is null for data-carrying enums (the common case).
        // A null comparison always produces null under SQL three-valued logic,
        // so it gets pruned just like a false branch.
        if !matches!(*match_expr.else_expr, Expr::Value(stmt::Value::Null)) {
            todo!("non-null else arm in match elimination");
        }

        Expr::or_from_vec(operands)
    }
}

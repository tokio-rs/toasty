use super::Simplify;
use std::mem;
use toasty_core::stmt::{self, BinaryOp, Expr};

impl Simplify<'_> {
    pub(super) fn simplify_expr_and(&mut self, expr: &mut stmt::ExprAnd) -> Option<stmt::Expr> {
        // Flatten any nested ands
        for i in 0..expr.operands.len() {
            if let stmt::Expr::And(and) = &mut expr.operands[i] {
                let mut nested = mem::take(&mut and.operands);
                expr.operands[i] = true.into();
                expr.operands.append(&mut nested);
            }
        }

        // `and(..., false, ...) → false`
        if expr.operands.iter().any(|e| e.is_false()) {
            return Some(false.into());
        }

        // `and(..., true, ...) → and(..., ...)`
        expr.operands.retain(|expr| !expr.is_true());

        // Null propagation, `null and null` → `null`
        // After removing true values, if all operands are null, return null.
        if !expr.operands.is_empty() && expr.operands.iter().all(|e| e.is_value_null()) {
            return Some(Expr::null());
        }

        // Idempotent law, `a and a` → `a`
        // Note: O(n) lookups are acceptable here since operand lists are typically small.
        let mut seen = Vec::new();
        expr.operands.retain(|operand| {
            if seen.contains(operand) {
                false
            } else {
                seen.push(operand.clone());
                true
            }
        });

        // Absorption law, `x and (x or y)` → `x`
        // If an operand is an OR that contains another operand of the AND, remove the OR.
        let non_or_operands: Vec<_> = expr
            .operands
            .iter()
            .filter(|op| !matches!(op, stmt::Expr::Or(_)))
            .cloned()
            .collect();

        expr.operands.retain(|operand| {
            if let stmt::Expr::Or(or_expr) = operand {
                // Remove this OR if any of its operands appears as a direct operand of the AND
                !or_expr
                    .operands
                    .iter()
                    .any(|op| non_or_operands.contains(op))
            } else {
                true
            }
        });

        // Complement law, `a and not(a)` → `false` (only if `a` is non-nullable)
        if self.try_complement_and(expr) {
            return Some(false.into());
        }

        // Range to equality: `a >= c and a <= c` → `a = c`
        self.try_range_to_equality(expr);

        if expr.operands.is_empty() {
            Some(true.into())
        } else if expr.operands.len() == 1 {
            Some(expr.operands.remove(0))
        } else {
            None
        }
    }

    /// Checks for complement law: `a and not(a)` → `false`
    /// Returns true if a complementary pair is found and both are non-nullable.
    fn try_complement_and(&self, expr: &stmt::ExprAnd) -> bool {
        // Collect all NOT expressions and their inner expressions
        let negated: Vec<_> = expr
            .operands
            .iter()
            .filter_map(|op| {
                if let stmt::Expr::Not(not_expr) = op {
                    Some(not_expr.expr.as_ref())
                } else {
                    None
                }
            })
            .collect();

        // Check if any operand has its negation also present
        for operand in &expr.operands {
            // Skip NOT expressions themselves
            if matches!(operand, stmt::Expr::Not(_)) {
                continue;
            }

            // Check if not(operand) exists and operand is non-nullable
            if negated.contains(&operand) && operand.is_always_non_nullable() {
                return true;
            }
        }

        false
    }

    /// Finds pairs of range comparisons that collapse to equality.
    ///
    /// `a >= c and a <= c` → `a = c`
    ///
    /// When a pair is found, all other bounds on the same (lhs, rhs) are also
    /// removed since equality implies them.
    ///
    /// NOTE: This assumes comparisons are already canonicalized with literals
    /// on the right-hand side (e.g., `a >= 5` not `5 <= a`).
    fn try_range_to_equality(&mut self, expr: &mut stmt::ExprAnd) {
        for i in 0..expr.operands.len() {
            let Expr::BinaryOp(op_i) = &expr.operands[i] else {
                continue;
            };

            if !matches!(op_i.op, BinaryOp::Ge | BinaryOp::Le) {
                continue;
            }

            for j in (i + 1)..expr.operands.len() {
                let Expr::BinaryOp(op_j) = &expr.operands[j] else {
                    continue;
                };

                if !matches!(
                    (op_i.op, op_j.op),
                    (BinaryOp::Ge, BinaryOp::Le) | (BinaryOp::Le, BinaryOp::Ge)
                ) {
                    continue;
                }

                if op_i.lhs == op_j.lhs && op_i.rhs == op_j.rhs {
                    let lhs = op_i.lhs.clone();
                    let rhs = op_i.rhs.clone();

                    // Replace the first operand with equality
                    expr.operands[i] = Expr::eq(lhs.as_ref().clone(), rhs.as_ref().clone());

                    // Mark all other `Ge`/`Le` bounds on the same (lhs, rhs)
                    // for removal
                    for k in (i + 1)..expr.operands.len() {
                        if let Expr::BinaryOp(op_k) = &expr.operands[k] {
                            if matches!(op_k.op, BinaryOp::Ge | BinaryOp::Le)
                                && op_k.lhs == lhs
                                && op_k.rhs == rhs
                            {
                                expr.operands[k] = true.into();
                            }
                        }
                    }
                    break;
                }
            }
        }

        expr.operands.retain(|e| !e.is_true());
    }
}

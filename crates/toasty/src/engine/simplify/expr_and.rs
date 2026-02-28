use super::Simplify;
use std::mem;
use toasty_core::stmt::{self, BinaryOp, Expr};

/// Collected `(lhs, value)` pairs from `==` and `!=` comparisons: `(eqs, nes)`.
type EqNePairs<'a> = (Vec<(&'a Expr, &'a stmt::Value)>, Vec<(&'a Expr, &'a stmt::Value)>);

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

        // Contradicting equality check + OR branch pruning.
        //
        // Extracts eq/ne constraints from non-OR operands, then:
        // 1. Checks for flat contradictions (e.g. `a == 1 AND a == 2` → false)
        // 2. Prunes OR branches that contradict the constraints (e.g.
        //    `AND(x == 1, OR(AND(x == 1, a), AND(x != 1, b)))` → `AND(x == 1, a)`)
        if let Some(result) = Self::check_contradictions(expr) {
            return Some(result);
        }

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

    /// Checks for contradicting equality constraints among the AND's operands,
    /// and prunes OR branches that are contradicted by non-OR operands.
    ///
    /// Extracts `(lhs, value)` pairs from eq/ne comparisons once, then uses
    /// them for both:
    /// 1. Flat contradiction detection (`a == 1 AND a == 2` → false)
    /// 2. OR branch pruning (`AND(x == 1, OR(AND(x != 1, b), ...))` → prune)
    ///
    /// Returns `Some(false)` if a flat contradiction or fully-pruned OR is
    /// found; otherwise mutates `expr` in place and returns `None`.
    fn check_contradictions(expr: &mut stmt::ExprAnd) -> Option<Expr> {
        // Separate OR operands from non-OR constraints.
        let mut or_operands: Vec<Expr> = Vec::new();
        for op in mem::take(&mut expr.operands) {
            if matches!(&op, Expr::Or(_)) {
                or_operands.push(op);
            } else {
                expr.operands.push(op);
            }
        }

        // Extract eq/ne pairs from the non-OR constraints.
        let constraints = Self::collect_eq_ne(&expr.operands);

        // Check for flat contradictions among the constraints.
        if Self::eq_ne_sets_contradict(&constraints, &constraints) {
            return Some(false.into());
        }

        // Prune OR branches that contradict the constraints.
        for or_op in &mut or_operands {
            let Expr::Or(or_expr) = or_op else {
                unreachable!()
            };

            or_expr.operands.retain(|branch| {
                let branch_ops: &[Expr] = match branch {
                    Expr::And(and) => &and.operands,
                    other => std::slice::from_ref(other),
                };
                let branch_eq_ne = Self::collect_eq_ne(branch_ops);
                !Self::eq_ne_sets_contradict(&constraints, &branch_eq_ne)
            });

            match or_expr.operands.len() {
                0 => *or_op = false.into(),
                1 => *or_op = or_expr.operands.remove(0),
                _ => {}
            }
        }

        // Put OR operands back, flattening surviving AND branches.
        for op in or_operands {
            match op {
                Expr::And(and) => expr.operands.extend(and.operands),
                other => expr.operands.push(other),
            }
        }

        // Re-check for false (all OR branches pruned → false).
        if expr.operands.iter().any(|e| e.is_false()) {
            return Some(false.into());
        }

        // Deduplicate after flattening.
        let mut seen = Vec::new();
        expr.operands.retain(|operand| {
            if seen.contains(operand) {
                false
            } else {
                seen.push(operand.clone());
                true
            }
        });

        None
    }

    /// Extracts `(lhs, value)` pairs from `==` and `!=` comparisons.
    fn collect_eq_ne(operands: &[Expr]) -> EqNePairs<'_> {
        let mut eqs = Vec::new();
        let mut nes = Vec::new();

        for op in operands {
            if let Expr::BinaryOp(binop) = op {
                if let Expr::Value(val) = binop.rhs.as_ref() {
                    match binop.op {
                        BinaryOp::Eq => eqs.push((binop.lhs.as_ref(), val)),
                        BinaryOp::Ne => nes.push((binop.lhs.as_ref(), val)),
                        _ => {}
                    }
                }
            }
        }

        (eqs, nes)
    }

    /// Returns `true` if two sets of eq/ne constraints contradict each other:
    ///
    /// - `a == 1` in one set and `a == 2` in the other
    /// - `a == 1` in one set and `a != 1` in the other
    fn eq_ne_sets_contradict(a: &EqNePairs<'_>, b: &EqNePairs<'_>) -> bool {
        let (a_eqs, a_nes) = a;
        let (b_eqs, b_nes) = b;

        for (a_lhs, a_val) in a_eqs {
            for (b_lhs, b_val) in b_eqs {
                if a_lhs == b_lhs && a_val != b_val {
                    return true;
                }
            }
        }

        for (eq_lhs, eq_val) in a_eqs.iter().chain(b_eqs.iter()) {
            for (ne_lhs, ne_val) in a_nes.iter().chain(b_nes.iter()) {
                if eq_lhs == ne_lhs && eq_val == ne_val {
                    return true;
                }
            }
        }

        false
    }
}

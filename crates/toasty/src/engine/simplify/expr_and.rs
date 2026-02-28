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

        // Contradicting equality: `a == c1 and a == c2` → `false` when c1 != c2
        if Self::has_contradicting_equality(&expr.operands) {
            return Some(false.into());
        }

        // Range to equality: `a >= c and a <= c` → `a = c`
        self.try_range_to_equality(expr);

        // Prune OR branches contradicted by other AND operands.
        //
        // AND(x == 1, OR(AND(x == 1, a), AND(x != 1, b)))
        //   → prune second OR branch (x == 1 contradicts x != 1)
        //   → AND(x == 1, a)
        //
        // This arises after match elimination distributes a binary op over
        // match arms: the else branch gets a negated guard that contradicts
        // the outer discriminant check.
        self.prune_or_branches(expr);

        // Re-check for false after pruning (all OR branches removed → false).
        if expr.operands.iter().any(|e| e.is_false()) {
            return Some(false.into());
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

    /// Checks for contradicting equality constraints:
    ///
    /// - `a == c1 AND a == c2` where c1 != c2 → contradiction
    /// - `a == c AND a != c` → contradiction
    ///
    /// TODO: This runs O(n^2) on every AND node during the walk, which is
    /// wasteful — most AND nodes don't contain contradictions. This should move
    /// to a dedicated post-lowering pass that runs once against the stable
    /// predicate tree. See `docs/roadmap/query-engine.md`.
    fn has_contradicting_equality(operands: &[Expr]) -> bool {
        Self::has_cross_contradiction(operands, operands)
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

    /// For each OR child of this AND, prune branches that are contradicted by
    /// other AND operands.
    ///
    /// Given `AND(P, OR(B1, B2))`, tests each `Bi` by combining it with `P`.
    /// If the combined constraints contain a contradicting equality, `Bi` is
    /// provably false and is removed from the OR.
    ///
    /// After pruning, surviving branches are flattened back into the AND.
    fn prune_or_branches(&self, expr: &mut stmt::ExprAnd) {
        // Separate OR operands from non-OR constraints.
        let mut or_operands: Vec<Expr> = Vec::new();
        for op in mem::take(&mut expr.operands) {
            if matches!(&op, Expr::Or(_)) {
                or_operands.push(op);
            } else {
                expr.operands.push(op);
            }
        }

        if or_operands.is_empty() {
            return;
        }

        // Prune each OR's branches against the non-OR constraints.
        for or_op in &mut or_operands {
            let Expr::Or(or_expr) = or_op else {
                unreachable!()
            };

            or_expr.operands.retain(|branch| {
                !Self::branch_contradicts(&expr.operands, branch)
            });

            // Collapse after pruning.
            match or_expr.operands.len() {
                0 => *or_op = false.into(),
                1 => *or_op = or_expr.operands.remove(0),
                _ => {}
            }
        }

        // Put the (possibly simplified) OR operands back and flatten any
        // nested ANDs that resulted from unwrapping single-branch ORs.
        for op in or_operands {
            match op {
                Expr::And(and) => expr.operands.extend(and.operands),
                other => expr.operands.push(other),
            }
        }

        // Deduplicate: flattening may introduce duplicates (e.g. disc==1
        // from both the outer AND and the surviving OR branch).
        let mut seen = Vec::new();
        expr.operands.retain(|operand| {
            if seen.contains(operand) {
                false
            } else {
                seen.push(operand.clone());
                true
            }
        });
    }

    /// Returns `true` if `branch` combined with `constraints` contains a
    /// contradicting equality.
    fn branch_contradicts(constraints: &[Expr], branch: &Expr) -> bool {
        let branch_operands: &[Expr] = match branch {
            Expr::And(and) => &and.operands,
            other => std::slice::from_ref(other),
        };

        Self::has_cross_contradiction(constraints, branch_operands)
    }

    /// Checks whether two sets of operands, if ANDed together, would produce
    /// a contradiction. Specifically, returns `true` when:
    ///
    /// - `a == 1` appears in one set and `a == 2` in the other (conflicting equalities)
    /// - `a == 1` appears in one set and `a != 1` in the other (equality vs negation)
    fn has_cross_contradiction(a: &[Expr], b: &[Expr]) -> bool {
        fn collect_eq_ne(operands: &[Expr]) -> (Vec<(&Expr, &stmt::Value)>, Vec<(&Expr, &stmt::Value)>) {
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

        let (a_eqs, a_nes) = collect_eq_ne(a);
        let (b_eqs, b_nes) = collect_eq_ne(b);

        // a == c1 in one set, a == c2 in the other (c1 != c2)
        for (a_lhs, a_val) in &a_eqs {
            for (b_lhs, b_val) in &b_eqs {
                if a_lhs == b_lhs && a_val != b_val {
                    return true;
                }
            }
        }

        // a == c in one set, a != c in the other
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

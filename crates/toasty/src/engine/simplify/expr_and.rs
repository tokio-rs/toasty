use super::Simplify;
use std::mem;
use toasty_core::stmt::{self, BinaryOp, Expr};

impl Simplify<'_> {
    /// Heavyweight AND rewrites. Cheap canonicalization (flatten, drop unit
    /// literals, null propagation, single/empty collapse on canonical input)
    /// runs in `fold::expr_and` before this is reached.
    pub(super) fn simplify_expr_and(&mut self, expr: &mut stmt::ExprAnd) -> Option<stmt::Expr> {
        // Idempotent law, `a and a` → `a`
        // Note: O(n) lookups are acceptable here since operand lists are typically small.
        // `is_equivalent_to` (not `PartialEq`) keeps this sound for non-deterministic
        // operands like `LAST_INSERT_ID()` — two syntactically identical calls may
        // return different values, so the second occurrence must survive.
        let mut seen: Vec<Expr> = Vec::new();
        expr.operands.retain(|operand| {
            if seen.iter().any(|e| e.is_equivalent_to(operand)) {
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
                    .any(|op| non_or_operands.iter().any(|e| e.is_equivalent_to(op)))
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

        // Redundant prefix subsumed by equality:
        //   `a = "<prefix><...>" AND starts_with(a, "<prefix>")` → `a = "<prefix><...>"`
        // The equality is strictly more selective. Two predicates targeting
        // the same column collide on DynamoDB's KeyConditionExpression
        // (one condition per key), and even on SQL backends the begins_with
        // is dead weight. Used by the IC discriminator pipeline to elide
        // the `StartsWith(sort_col, "<Model>#")` marker against a fully
        // qualified `sort_col = "<Model>#<id>"`.
        prune_starts_with_subsumed_by_eq(expr);

        // Redundant prefix subsumed by a longer prefix:
        //   `starts_with(a, "<long>") AND starts_with(a, "<short>")`
        //     → `starts_with(a, "<long>")` when `<long>` begins with `<short>`.
        // Same DDB collision concern as the eq case above. Hits the IC
        // parent→child read shape, where `lower::association` emits a
        // hierarchical `StartsWith(sk, "<Child>#<chain>#")` and
        // `apply_lowering_filter_constraint` independently emits the model
        // discriminator `StartsWith(sk, "<Child>#")`. Both are correct; the
        // shorter is implied by the longer.
        prune_starts_with_subsumed_by_starts_with(expr);

        // Contradicting equality: `a == 1 AND a == 2` → false,
        // `a == 1 AND a != 1` → false
        if has_self_contradiction(&expr.operands) {
            return Some(false.into());
        }

        // OR branch pruning: `AND(x == 1, OR(AND(x != 1, b), ...))` → prune
        // branches whose eq/ne constraints contradict the outer AND constraints.
        if let Some(result) = prune_or_branches(expr) {
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
            if negated.iter().any(|n| n.is_equivalent_to(operand))
                && operand.is_always_non_nullable()
            {
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

                if op_i.lhs.is_equivalent_to(&op_j.lhs) && op_i.rhs.is_equivalent_to(&op_j.rhs) {
                    let lhs = op_i.lhs.clone();
                    let rhs = op_i.rhs.clone();

                    // Replace the first operand with equality
                    expr.operands[i] = Expr::eq(lhs.as_ref().clone(), rhs.as_ref().clone());

                    // Mark all other `Ge`/`Le` bounds on the same (lhs, rhs)
                    // for removal
                    for k in (i + 1)..expr.operands.len() {
                        if let Expr::BinaryOp(op_k) = &expr.operands[k]
                            && matches!(op_k.op, BinaryOp::Ge | BinaryOp::Le)
                            && op_k.lhs.is_equivalent_to(&lhs)
                            && op_k.rhs.is_equivalent_to(&rhs)
                        {
                            expr.operands[k] = true.into();
                        }
                    }
                    break;
                }
            }
        }

        expr.operands.retain(|e| !e.is_true());
    }
}

/// Checks for contradicting equality constraints within a single operand
/// list: `a == 1 AND a == 2` → true, `a == 1 AND a != 1` → true.
fn has_self_contradiction(operands: &[Expr]) -> bool {
    for i in 0..operands.len() {
        if is_contradicting_eq_constraints(&operands[i..=i], &operands[i + 1..]) {
            return true;
        }
    }
    false
}

/// Prunes OR branches whose eq/ne constraints contradict the outer AND's
/// non-OR constraints. Returns `Some(false)` if pruning produces a false
/// operand; otherwise mutates `expr` in place and returns `None`.
fn prune_or_branches(expr: &mut stmt::ExprAnd) -> Option<Expr> {
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
        return None;
    }

    // Prune OR branches that contradict the outer constraints.
    for or_op in &mut or_operands {
        let Expr::Or(or_expr) = or_op else {
            unreachable!()
        };

        or_expr.operands.retain(|branch| {
            let branch_ops: &[Expr] = match branch {
                Expr::And(and) => &and.operands,
                other => std::slice::from_ref(other),
            };
            !is_contradicting_eq_constraints(&expr.operands, branch_ops)
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

    // All OR branches pruned → false.
    if expr.operands.iter().any(|e| e.is_false()) {
        return Some(false.into());
    }

    // Deduplicate after flattening (flatten can reintroduce operands
    // already present in the outer AND). `is_equivalent_to` skips dedup
    // of non-deterministic operands, preserving their independent evaluations.
    let mut seen: Vec<Expr> = Vec::new();
    expr.operands.retain(|operand| {
        if seen.iter().any(|e| e.is_equivalent_to(operand)) {
            false
        } else {
            seen.push(operand.clone());
            true
        }
    });

    None
}

/// Returns `true` if any eq/ne constraint in `a` contradicts any
/// eq/ne constraint in `b`.
fn is_contradicting_eq_constraints(a: &[Expr], b: &[Expr]) -> bool {
    for outer_op in a {
        let Some((o_lhs, o_op, o_val)) = extract_eq_ne(outer_op) else {
            continue;
        };

        for branch_op in b {
            let Some((b_lhs, b_op, b_val)) = extract_eq_ne(branch_op) else {
                continue;
            };

            // Only consider the two lhs sides "the same value" when they are
            // syntactically equal AND stable. Otherwise `f() == 1 AND f() == 2`
            // would be rewritten to `false`, but two evaluations of a
            // non-deterministic `f()` can produce 1 and 2.
            if !o_lhs.is_equivalent_to(b_lhs) {
                continue;
            }

            match (o_op, b_op) {
                (BinaryOp::Eq, BinaryOp::Eq) if o_val != b_val => return true,
                (BinaryOp::Eq, BinaryOp::Ne) | (BinaryOp::Ne, BinaryOp::Eq) if o_val == b_val => {
                    return true;
                }
                _ => {}
            }
        }
    }

    false
}

/// Drops `starts_with(col, prefix)` operands when another operand pins the
/// same column to a literal string that already starts with `prefix`. The
/// equality is strictly stronger; the begins_with is dead weight.
fn prune_starts_with_subsumed_by_eq(expr: &mut stmt::ExprAnd) {
    // Collect (lhs_expr, prefix_value) for every `col = "<literal>"` operand.
    let eq_literals: Vec<(Expr, String)> = expr
        .operands
        .iter()
        .filter_map(|op| {
            let Expr::BinaryOp(binop) = op else {
                return None;
            };
            if !matches!(binop.op, BinaryOp::Eq) {
                return None;
            }
            let Expr::Value(stmt::Value::String(s)) = binop.rhs.as_ref() else {
                return None;
            };
            Some(((*binop.lhs).clone(), s.clone()))
        })
        .collect();

    if eq_literals.is_empty() {
        return;
    }

    expr.operands.retain(|op| {
        let Expr::StartsWith(sw) = op else {
            return true;
        };
        let Expr::Value(stmt::Value::String(prefix)) = sw.prefix.as_ref() else {
            return true;
        };
        // Drop only if some sibling `col = "..."` pins the same column AND
        // its literal value already begins with this prefix.
        !eq_literals
            .iter()
            .any(|(eq_lhs, eq_val)| eq_lhs.is_equivalent_to(&sw.expr) && eq_val.starts_with(prefix))
    });
}

/// Drops `starts_with(col, short)` operands when another `starts_with(col, long)`
/// targets the same column with a longer prefix that already begins with
/// `short`. The longer prefix is strictly stronger.
fn prune_starts_with_subsumed_by_starts_with(expr: &mut stmt::ExprAnd) {
    // Collect (lhs_expr, prefix) for every `starts_with(col, "<literal>")` operand.
    let prefixes: Vec<(Expr, String)> = expr
        .operands
        .iter()
        .filter_map(|op| {
            let Expr::StartsWith(sw) = op else {
                return None;
            };
            let Expr::Value(stmt::Value::String(s)) = sw.prefix.as_ref() else {
                return None;
            };
            Some(((*sw.expr).clone(), s.clone()))
        })
        .collect();

    if prefixes.len() < 2 {
        return;
    }

    expr.operands.retain(|op| {
        let Expr::StartsWith(sw) = op else {
            return true;
        };
        let Expr::Value(stmt::Value::String(prefix)) = sw.prefix.as_ref() else {
            return true;
        };
        // Keep this operand unless some sibling `starts_with(col, "<longer>")`
        // pins the same column with a strictly-longer prefix that begins with
        // this one. Equal-length prefixes are handled by the AND's idempotency
        // pass; we only drop when there's a more-specific predicate to defer to.
        !prefixes.iter().any(|(other_lhs, other_val)| {
            other_lhs.is_equivalent_to(&sw.expr)
                && other_val.len() > prefix.len()
                && other_val.starts_with(prefix.as_str())
        })
    });
}

/// Extracts `(lhs, op, rhs_value)` from an `Expr::BinaryOp` if it is an
/// `==` or `!=` with a constant value on the right.
fn extract_eq_ne(expr: &Expr) -> Option<(&Expr, BinaryOp, &stmt::Value)> {
    if let Expr::BinaryOp(binop) = expr
        && let Expr::Value(val) = binop.rhs.as_ref()
        && matches!(binop.op, BinaryOp::Eq | BinaryOp::Ne)
    {
        return Some((binop.lhs.as_ref(), binop.op, val));
    }
    None
}

use super::Simplify;
use std::mem;
use toasty_core::stmt;

impl Simplify<'_> {
    pub(super) fn simplify_expr_or(&mut self, expr: &mut stmt::ExprOr) -> Option<stmt::Expr> {
        // Flatten any nested ors
        for i in 0..expr.operands.len() {
            if let stmt::Expr::Or(or) = &mut expr.operands[i] {
                let mut nested = mem::take(&mut or.operands);
                expr.operands[i] = false.into();
                expr.operands.append(&mut nested);
            }
        }

        // `or(..., true, ...) → true`
        if expr.operands.iter().any(|e| e.is_true()) {
            return Some(true.into());
        }

        // `or(..., false, ...) → or(..., ...)`
        expr.operands.retain(|expr| !expr.is_false());

        // Null propagation, `null or null` → `null`
        // After removing false values, if all operands are null, return null.
        if !expr.operands.is_empty() && expr.operands.iter().all(|e| e.is_value_null()) {
            return Some(stmt::Expr::null());
        }

        // Idempotent law, `a or a` → `a`
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

        // Absorption law, `x or (x and y)` → `x`
        // If an operand is an AND that contains another operand of the OR, remove the AND.
        let non_and_operands: Vec<_> = expr
            .operands
            .iter()
            .filter(|op| !matches!(op, stmt::Expr::And(_)))
            .cloned()
            .collect();

        expr.operands.retain(|operand| {
            if let stmt::Expr::And(and_expr) = operand {
                // Remove this AND if any of its operands appears as a direct operand of the OR
                !and_expr
                    .operands
                    .iter()
                    .any(|op| non_and_operands.contains(op))
            } else {
                true
            }
        });

        // Factoring, `(a and b) or (a and c)` → `a and (b or c)`
        // Find common factors across all AND operands and factor them out.
        if let Some(factored) = self.try_factor_or(expr) {
            return Some(factored);
        }

        // Complement law, `a or not(a)` → `true` (only if `a` is non-nullable)
        if self.try_complement_or(expr) {
            return Some(true.into());
        }

        // OR-to-IN conversion, `a = 1 or a = 2 or a = 3` → `a in (1, 2, 3)`
        if let Some(in_list) = self.try_or_to_in_list(expr) {
            return Some(in_list);
        }

        if expr.operands.is_empty() {
            Some(false.into())
        } else if expr.operands.len() == 1 {
            Some(expr.operands.remove(0))
        } else {
            None
        }
    }

    /// Attempts to factor common terms from AND expressions within an OR.
    /// `(a and b) or (a and c)` → `a and (b or c)`
    /// `(a and b and c) or (a and b and d)` → `a and b and (c or d)`
    fn try_factor_or(&self, expr: &mut stmt::ExprOr) -> Option<stmt::Expr> {
        // Need at least 2 operands, all must be ANDs
        if expr.operands.len() < 2 {
            return None;
        }

        if !expr
            .operands
            .iter()
            .all(|op| matches!(op, stmt::Expr::And(_)))
        {
            return None;
        }

        // Find all common factors by checking which operands from the first AND
        // appear in all other ANDs
        let first_and = match &expr.operands[0] {
            stmt::Expr::And(and) => and,
            _ => unreachable!(),
        };

        let common: Vec<_> = first_and
            .operands
            .iter()
            .filter(|op| {
                expr.operands[1..].iter().all(|other| {
                    if let stmt::Expr::And(other_and) = other {
                        other_and.operands.contains(op)
                    } else {
                        false
                    }
                })
            })
            .cloned()
            .collect();

        if common.is_empty() {
            return None;
        }

        // Remove all common factors from each AND
        for operand in &mut expr.operands {
            if let stmt::Expr::And(and) = operand {
                and.operands.retain(|op| !common.contains(op));
                // If only one operand left, unwrap the AND
                if and.operands.len() == 1 {
                    *operand = and.operands.pop().unwrap();
                } else if and.operands.is_empty() {
                    *operand = true.into();
                }
            }
        }

        // Common factors AND (the modified OR)
        let mut result = common;
        let or_expr = stmt::ExprOr {
            operands: mem::take(&mut expr.operands),
        };
        result.push(stmt::Expr::Or(or_expr));
        Some(stmt::Expr::and_from_vec(result))
    }

    /// Checks for complement law: `a or not(a)` → `true`
    ///
    /// Returns true if a complementary pair is found and both are non-nullable.
    fn try_complement_or(&self, expr: &stmt::ExprOr) -> bool {
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

    /// Converts disjunctive equality chains to IN lists.
    ///
    /// `a = 1 or a = 2 or b = 3` → `a in (1, 2) or b = 3`
    /// `a = 1 or a = 2 or b = 3 or b = 4` → `a in (1, 2) or b in (3, 4)`
    ///
    /// Groups equality comparisons by their LHS and converts groups with 2+
    /// values into IN lists. Non-equality operands are preserved.
    fn try_or_to_in_list(&self, expr: &mut stmt::ExprOr) -> Option<stmt::Expr> {
        use std::collections::HashMap;

        // Group equalities by their LHS expression
        // Key: index into `lhs_exprs`, Value: list of RHS constant values
        //
        // TODO: this could be simplified to use `Expr` as the `HashMap` key
        // directly if `Expr` ever implements `Hash`.
        let mut lhs_exprs: Vec<stmt::Expr> = Vec::new();
        let mut groups: HashMap<usize, Vec<stmt::Value>> = HashMap::new();
        let mut other_operands: Vec<stmt::Expr> = Vec::new();

        for operand in mem::take(&mut expr.operands) {
            if let stmt::Expr::BinaryOp(bin_op) = &operand {
                if bin_op.op.is_eq() {
                    if let stmt::Expr::Value(value) = bin_op.rhs.as_ref() {
                        // Find or create index for this LHS
                        let lhs_idx = lhs_exprs
                            .iter()
                            .position(|e| e == bin_op.lhs.as_ref())
                            .unwrap_or_else(|| {
                                lhs_exprs.push(bin_op.lhs.as_ref().clone());
                                lhs_exprs.len() - 1
                            });

                        groups.entry(lhs_idx).or_default().push(value.clone());
                        continue;
                    }
                }
            }

            // Non-equality or non-constant RHS - keep as is
            other_operands.push(operand);
        }

        // Check if any conversion will happen (any group with 2+ values)
        let has_conversion = groups.values().any(|v| v.len() >= 2);
        if !has_conversion {
            // Restore original operands and return None
            for (idx, values) in groups {
                let lhs = &lhs_exprs[idx];
                for value in values {
                    other_operands.push(stmt::Expr::eq(lhs.clone(), stmt::Expr::Value(value)));
                }
            }
            expr.operands = other_operands;
            return None;
        }

        // Build result operands
        let mut result_operands = other_operands;

        for (idx, values) in groups {
            let lhs = lhs_exprs[idx].clone();
            if values.len() >= 2 {
                // Convert to IN list
                result_operands.push(stmt::Expr::in_list(lhs, stmt::Expr::list(values)));
            } else {
                // Keep as equality
                for value in values {
                    result_operands.push(stmt::Expr::eq(lhs.clone(), stmt::Expr::Value(value)));
                }
            }
        }

        if result_operands.len() == 1 {
            Some(result_operands.remove(0))
        } else {
            expr.operands = result_operands;
            None
        }
    }
}

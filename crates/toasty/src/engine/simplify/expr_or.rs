use super::{Simplify, dedup_operands, has_complement};
use std::mem;
use toasty_core::stmt;

impl Simplify<'_> {
    /// Heavyweight OR rewrites. Cheap canonicalization (flatten, true
    /// short-circuit, drop false, null propagation) runs in `fold::expr_or`
    /// before this is reached.
    pub(super) fn simplify_expr_or(&mut self, expr: &mut stmt::ExprOr) -> Option<stmt::Expr> {
        dedup_operands(&mut expr.operands);

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
                    .any(|op| non_and_operands.iter().any(|e| e.is_equivalent_to(op)))
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
        if has_complement(&expr.operands) {
            return Some(true.into());
        }

        // The variant-tautology rewrite (`is_variant(x, 0) or is_variant(x, 1)`
        // covering all variants → `true`) fires in the pre-lowering
        // `LowerStatement::visit_expr_mut` `Expr::Or` arm, not here.

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
                        other_and.operands.iter().any(|e| e.is_equivalent_to(op))
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
                and.operands
                    .retain(|op| !common.iter().any(|c| c.is_equivalent_to(op)));
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

    /// Converts disjunctive equality chains to IN lists.
    ///
    /// `a = 1 or a = 2 or b = 3` → `a in (1, 2) or b = 3`
    /// `a = 1 or a = 2 or b = 3 or b = 4` → `a in (1, 2) or b in (3, 4)`
    ///
    /// Groups equality comparisons by their LHS and converts groups with 2+
    /// values into IN lists. Non-equality operands are preserved.
    fn try_or_to_in_list(&self, expr: &mut stmt::ExprOr) -> Option<stmt::Expr> {
        let mut groups: Vec<(stmt::Expr, Vec<stmt::Value>)> = Vec::new();
        let mut other_operands: Vec<stmt::Expr> = Vec::new();

        for operand in mem::take(&mut expr.operands) {
            if let stmt::Expr::BinaryOp(bin_op) = &operand
                && bin_op.op.is_eq()
                && let stmt::Expr::Value(value) = bin_op.rhs.as_ref()
            {
                // Keep non-deterministic expressions in separate groups so an
                // IN list cannot collapse independent evaluations into one.
                if let Some((_, values)) = groups
                    .iter_mut()
                    .find(|(lhs, _)| lhs.is_equivalent_to(&bin_op.lhs))
                {
                    values.push(value.clone());
                } else {
                    groups.push(((*bin_op.lhs).clone(), vec![value.clone()]));
                }
                continue;
            }

            // Non-equality or non-constant RHS - keep as is
            other_operands.push(operand);
        }

        let has_conversion = groups.iter().any(|(_, values)| values.len() >= 2);
        let mut result_operands = other_operands;

        for (lhs, mut values) in groups {
            let operand = if values.len() >= 2 {
                stmt::Expr::in_list(lhs, stmt::Expr::list(values))
            } else {
                stmt::Expr::eq(lhs, values.pop().unwrap())
            };
            result_operands.push(operand);
        }

        if has_conversion && result_operands.len() == 1 {
            Some(result_operands.remove(0))
        } else {
            expr.operands = result_operands;
            None
        }
    }
}

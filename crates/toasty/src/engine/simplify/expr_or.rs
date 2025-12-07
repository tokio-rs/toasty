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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::simplify::test::test_schema;
    use toasty_core::stmt::{Expr, ExprOr};

    /// Builds `or(a, or(b, c))`, a nested OR structure for testing flattening.
    fn nested_or(a: Expr, b: Expr, c: Expr) -> ExprOr {
        ExprOr {
            operands: vec![
                a,
                Expr::Or(ExprOr {
                    operands: vec![b, c],
                }),
            ],
        }
    }

    #[test]
    fn flatten_all_symbolic() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `or(A, or(B, C)) → or(A, B, C)`
        let mut expr = nested_or(Expr::arg(0), Expr::arg(1), Expr::arg(2));
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_none());
        assert_eq!(expr.operands.len(), 3);
        assert_eq!(expr.operands[0], Expr::arg(0));
        assert_eq!(expr.operands[1], Expr::arg(1));
        assert_eq!(expr.operands[2], Expr::arg(2));
    }

    #[test]
    fn flatten_with_false_in_outer() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `or(false, or(B, C)) → or(B, C)`
        let mut expr = nested_or(false.into(), Expr::arg(1), Expr::arg(2));
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_none());
        assert_eq!(expr.operands.len(), 2);
        assert_eq!(expr.operands[0], Expr::arg(1));
        assert_eq!(expr.operands[1], Expr::arg(2));
    }

    #[test]
    fn flatten_with_false_in_nested_first() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `or(A, or(false, C)) → or(A, C)`
        let mut expr = nested_or(Expr::arg(0), false.into(), Expr::arg(2));
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_none());
        assert_eq!(expr.operands.len(), 2);
        assert_eq!(expr.operands[0], Expr::arg(0));
        assert_eq!(expr.operands[1], Expr::arg(2));
    }

    #[test]
    fn flatten_outer_false_nested_one_false() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `or(false, or(false, C)) → C`
        let mut expr = nested_or(false.into(), false.into(), Expr::arg(2));
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), Expr::arg(2));
    }

    #[test]
    fn flatten_outer_symbolic_nested_all_false() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `or(A, or(false, false)) → A`
        let mut expr = nested_or(Expr::arg(0), false.into(), false.into());
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), Expr::arg(0));
    }

    #[test]
    fn flatten_all_false() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `or(false, or(false, false)) → false`
        let mut expr = nested_or(false.into(), false.into(), false.into());
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_false());
    }

    #[test]
    fn flatten_with_true_in_outer() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `or(true, or(B, C)) → true`
        let mut expr = nested_or(true.into(), Expr::arg(1), Expr::arg(2));
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_true());
    }

    #[test]
    fn flatten_with_true_in_nested() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `or(A, or(true, C)) → true`
        let mut expr = nested_or(Expr::arg(0), true.into(), Expr::arg(2));
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_true());
    }

    #[test]
    fn single_operand_unwrapped() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `or(arg(0)) → arg(0)`
        let mut expr = ExprOr {
            operands: vec![Expr::arg(0)],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), Expr::arg(0));
    }

    #[test]
    fn idempotent_two_identical() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `or(a, a) → a`
        let mut expr = ExprOr {
            operands: vec![Expr::arg(0), Expr::arg(0)],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), Expr::arg(0));
    }

    #[test]
    fn idempotent_three_identical() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `or(a, a, a) → a`
        let mut expr = ExprOr {
            operands: vec![Expr::arg(0), Expr::arg(0), Expr::arg(0)],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), Expr::arg(0));
    }

    #[test]
    fn idempotent_with_different() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `or(a, b, a) → or(a, b)`
        let mut expr = ExprOr {
            operands: vec![Expr::arg(0), Expr::arg(1), Expr::arg(0)],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_none());
        assert_eq!(expr.operands.len(), 2);
        assert_eq!(expr.operands[0], Expr::arg(0));
        assert_eq!(expr.operands[1], Expr::arg(1));
    }

    #[test]
    fn absorption_or_and() {
        use toasty_core::stmt::ExprAnd;

        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `or(a, and(a, b))` → `a`
        let mut expr = ExprOr {
            operands: vec![
                Expr::arg(0),
                Expr::And(ExprAnd {
                    operands: vec![Expr::arg(0), Expr::arg(1)],
                }),
            ],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), Expr::arg(0));
    }

    #[test]
    fn absorption_with_multiple_operands() {
        use toasty_core::stmt::ExprAnd;

        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `or(a, b, and(a, c))` → `or(a, b)`
        let mut expr = ExprOr {
            operands: vec![
                Expr::arg(0),
                Expr::arg(1),
                Expr::And(ExprAnd {
                    operands: vec![Expr::arg(0), Expr::arg(2)],
                }),
            ],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_none());
        assert_eq!(expr.operands.len(), 2);
        assert_eq!(expr.operands[0], Expr::arg(0));
        assert_eq!(expr.operands[1], Expr::arg(1));
    }

    #[test]
    fn absorption_two_or_three_and() {
        use toasty_core::stmt::ExprAnd;

        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `or(a, b, and(a, c, d))` → `or(a, b)`
        let mut expr = ExprOr {
            operands: vec![
                Expr::arg(0),
                Expr::arg(1),
                Expr::And(ExprAnd {
                    operands: vec![Expr::arg(0), Expr::arg(2), Expr::arg(3)],
                }),
            ],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_none());
        assert_eq!(expr.operands.len(), 2);
        assert_eq!(expr.operands[0], Expr::arg(0));
        assert_eq!(expr.operands[1], Expr::arg(1));
    }

    #[test]
    fn factoring_basic() {
        use toasty_core::stmt::ExprAnd;

        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `(a and b) or (a and c)` → `a and (b or c)`
        let mut expr = ExprOr {
            operands: vec![
                Expr::And(ExprAnd {
                    operands: vec![Expr::arg(0), Expr::arg(1)],
                }),
                Expr::And(ExprAnd {
                    operands: vec![Expr::arg(0), Expr::arg(2)],
                }),
            ],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        // Result should be `a and (b or c)`
        let Some(Expr::And(and_expr)) = result else {
            panic!("expected And");
        };
        assert_eq!(and_expr.operands.len(), 2);
        assert_eq!(and_expr.operands[0], Expr::arg(0));

        let Expr::Or(or_expr) = &and_expr.operands[1] else {
            panic!("expected Or");
        };
        assert_eq!(or_expr.operands.len(), 2);
        assert_eq!(or_expr.operands[0], Expr::arg(1));
        assert_eq!(or_expr.operands[1], Expr::arg(2));
    }

    #[test]
    fn factoring_multiple_common() {
        use toasty_core::stmt::ExprAnd;

        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `(a and b and c) or (a and b and d)` → `a and b and (c or d)`
        let mut expr = ExprOr {
            operands: vec![
                Expr::And(ExprAnd {
                    operands: vec![Expr::arg(0), Expr::arg(1), Expr::arg(2)],
                }),
                Expr::And(ExprAnd {
                    operands: vec![Expr::arg(0), Expr::arg(1), Expr::arg(3)],
                }),
            ],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        // Result should be `a and b and (c or d)`
        let Some(Expr::And(and_expr)) = result else {
            panic!("expected And");
        };
        assert_eq!(and_expr.operands.len(), 3);
        assert_eq!(and_expr.operands[0], Expr::arg(0));
        assert_eq!(and_expr.operands[1], Expr::arg(1));

        let Expr::Or(or_expr) = &and_expr.operands[2] else {
            panic!("expected Or");
        };
        assert_eq!(or_expr.operands.len(), 2);
        assert_eq!(or_expr.operands[0], Expr::arg(2));
        assert_eq!(or_expr.operands[1], Expr::arg(3));
    }

    #[test]
    fn factoring_three_ands() {
        use toasty_core::stmt::ExprAnd;

        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `(a and b) or (a and c) or (a and d)` → `a and (b or c or d)`
        let mut expr = ExprOr {
            operands: vec![
                Expr::And(ExprAnd {
                    operands: vec![Expr::arg(0), Expr::arg(1)],
                }),
                Expr::And(ExprAnd {
                    operands: vec![Expr::arg(0), Expr::arg(2)],
                }),
                Expr::And(ExprAnd {
                    operands: vec![Expr::arg(0), Expr::arg(3)],
                }),
            ],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        // Result should be `a and (b or c or d)`
        let Some(Expr::And(and_expr)) = result else {
            panic!("expected And");
        };
        assert_eq!(and_expr.operands.len(), 2);
        assert_eq!(and_expr.operands[0], Expr::arg(0));

        let Expr::Or(or_expr) = &and_expr.operands[1] else {
            panic!("expected Or");
        };
        assert_eq!(or_expr.operands.len(), 3);
        assert_eq!(or_expr.operands[0], Expr::arg(1));
        assert_eq!(or_expr.operands[1], Expr::arg(2));
        assert_eq!(or_expr.operands[2], Expr::arg(3));
    }

    #[test]
    fn factoring_no_common() {
        use toasty_core::stmt::ExprAnd;

        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `(a and b) or (c and d)` → no change (no common factor)
        let mut expr = ExprOr {
            operands: vec![
                Expr::And(ExprAnd {
                    operands: vec![Expr::arg(0), Expr::arg(1)],
                }),
                Expr::And(ExprAnd {
                    operands: vec![Expr::arg(2), Expr::arg(3)],
                }),
            ],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_none());
    }

    #[test]
    fn complement_basic() {
        use toasty_core::stmt::ExprNot;

        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `a or not(a)` → `true` (where a is a non-nullable comparison)
        let a = Expr::eq(Expr::arg(0), Expr::arg(1));
        let mut expr = ExprOr {
            operands: vec![a.clone(), Expr::Not(ExprNot { expr: Box::new(a) })],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_true());
    }

    #[test]
    fn complement_with_other_operands() {
        use toasty_core::stmt::ExprNot;

        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `a or b or not(a)` → `true`
        let a = Expr::eq(Expr::arg(0), Expr::arg(1));
        let mut expr = ExprOr {
            operands: vec![
                a.clone(),
                Expr::arg(2),
                Expr::Not(ExprNot { expr: Box::new(a) }),
            ],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_true());
    }

    #[test]
    fn complement_nullable_not_simplified() {
        use toasty_core::stmt::ExprNot;

        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `a or not(a)` where `a` is an arg (nullable) → no change
        let a = Expr::arg(0);
        let mut expr = ExprOr {
            operands: vec![a.clone(), Expr::Not(ExprNot { expr: Box::new(a) })],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_none());
    }

    #[test]
    fn complement_multiple_repetitions() {
        use toasty_core::stmt::ExprNot;

        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `a or a or not(a) or not(a)` → `true`
        let a = Expr::eq(Expr::arg(0), Expr::arg(1));
        let mut expr = ExprOr {
            operands: vec![
                a.clone(),
                a.clone(),
                Expr::Not(ExprNot {
                    expr: Box::new(a.clone()),
                }),
                Expr::Not(ExprNot { expr: Box::new(a) }),
            ],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_true());
    }

    // Null propagation tests

    #[test]
    fn null_or_null_becomes_null() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `null or null` → `null`
        let mut expr = ExprOr {
            operands: vec![Expr::null(), Expr::null()],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_value_null());
    }

    #[test]
    fn null_or_false_becomes_null() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `null or false` → `null` (false is removed, leaving only null)
        let mut expr = ExprOr {
            operands: vec![Expr::null(), false.into()],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_value_null());
    }

    #[test]
    fn false_or_null_becomes_null() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `false or null` → `null` (false is removed, leaving only null)
        let mut expr = ExprOr {
            operands: vec![false.into(), Expr::null()],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_value_null());
    }

    #[test]
    fn null_or_true_becomes_true() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `null or true` → `true`
        let mut expr = ExprOr {
            operands: vec![Expr::null(), true.into()],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_true());
    }

    #[test]
    fn null_or_symbolic_not_simplified() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `null or a` → no change (symbolic operand present)
        let mut expr = ExprOr {
            operands: vec![Expr::null(), Expr::arg(0)],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_none());
        assert_eq!(expr.operands.len(), 2);
    }

    #[test]
    fn multiple_nulls_become_null() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `null or null or null` → `null`
        let mut expr = ExprOr {
            operands: vec![Expr::null(), Expr::null(), Expr::null()],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_value_null());
    }

    #[test]
    fn multiple_nulls_and_false_becomes_null() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `null or false or null or false` → `null`
        let mut expr = ExprOr {
            operands: vec![Expr::null(), false.into(), Expr::null(), false.into()],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_value_null());
    }

    // OR-to-IN conversion tests

    #[test]
    fn or_to_in_basic() {
        use toasty_core::stmt::Value;

        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `a = 1 or a = 2 or a = 3` → `a in (1, 2, 3)`
        let mut expr = ExprOr {
            operands: vec![
                Expr::eq(Expr::arg(0), Expr::Value(Value::from(1i64))),
                Expr::eq(Expr::arg(0), Expr::Value(Value::from(2i64))),
                Expr::eq(Expr::arg(0), Expr::Value(Value::from(3i64))),
            ],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_some());
        let Some(Expr::InList(in_list)) = result else {
            panic!("expected InList");
        };
        assert_eq!(*in_list.expr, Expr::arg(0));
    }

    #[test]
    fn or_to_in_two_values() {
        use toasty_core::stmt::Value;

        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `a = 1 or a = 2` → `a in (1, 2)`
        let mut expr = ExprOr {
            operands: vec![
                Expr::eq(Expr::arg(0), Expr::Value(Value::from(1i64))),
                Expr::eq(Expr::arg(0), Expr::Value(Value::from(2i64))),
            ],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        assert!(result.is_some());
        assert!(matches!(result, Some(Expr::InList(_))));
    }

    #[test]
    fn or_to_in_single_value_not_converted() {
        use toasty_core::stmt::Value;

        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `a = 1` (single equality, not converted)
        let mut expr = ExprOr {
            operands: vec![Expr::eq(Expr::arg(0), Expr::Value(Value::from(1i64)))],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        // Single operand gets unwrapped, but not to IN
        assert!(result.is_some());
        assert!(matches!(result, Some(Expr::BinaryOp(_))));
    }

    #[test]
    fn or_to_in_different_lhs_not_converted() {
        use toasty_core::stmt::Value;

        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `a = 1 or b = 2` (different LHS, not converted to single IN)
        let mut expr = ExprOr {
            operands: vec![
                Expr::eq(Expr::arg(0), Expr::Value(Value::from(1i64))),
                Expr::eq(Expr::arg(1), Expr::Value(Value::from(2i64))),
            ],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        // Not converted, stays as OR
        assert!(result.is_none());
        assert_eq!(expr.operands.len(), 2);
    }

    #[test]
    fn or_to_in_mixed_keeps_other_operands() {
        use toasty_core::stmt::Value;

        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `a = 1 or a = 2 or b = 3` → `a in (1, 2) or b = 3`
        let mut expr = ExprOr {
            operands: vec![
                Expr::eq(Expr::arg(0), Expr::Value(Value::from(1i64))),
                Expr::eq(Expr::arg(0), Expr::Value(Value::from(2i64))),
                Expr::eq(Expr::arg(1), Expr::Value(Value::from(3i64))),
            ],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        // Stays as OR but with transformed operands
        assert!(result.is_none());
        assert_eq!(expr.operands.len(), 2);

        // Should have one InList and one BinaryOp
        let has_in_list = expr.operands.iter().any(|e| matches!(e, Expr::InList(_)));
        let has_binary_op = expr.operands.iter().any(|e| matches!(e, Expr::BinaryOp(_)));
        assert!(has_in_list);
        assert!(has_binary_op);
    }

    #[test]
    fn or_to_in_multiple_groups() {
        use toasty_core::stmt::Value;

        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `a = 1 or a = 2 or b = 3 or b = 4` → `a in (1, 2) or b in (3, 4)`
        let mut expr = ExprOr {
            operands: vec![
                Expr::eq(Expr::arg(0), Expr::Value(Value::from(1i64))),
                Expr::eq(Expr::arg(0), Expr::Value(Value::from(2i64))),
                Expr::eq(Expr::arg(1), Expr::Value(Value::from(3i64))),
                Expr::eq(Expr::arg(1), Expr::Value(Value::from(4i64))),
            ],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        // Stays as OR with two InList operands
        assert!(result.is_none());
        assert_eq!(expr.operands.len(), 2);

        // Both should be InList
        assert!(expr.operands.iter().all(|e| matches!(e, Expr::InList(_))));
    }

    #[test]
    fn or_to_in_with_non_equality_preserved() {
        use toasty_core::stmt::Value;

        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `a = 1 or a = 2 or c` → `a in (1, 2) or c`
        let mut expr = ExprOr {
            operands: vec![
                Expr::eq(Expr::arg(0), Expr::Value(Value::from(1i64))),
                Expr::eq(Expr::arg(0), Expr::Value(Value::from(2i64))),
                Expr::arg(2), // non-equality operand
            ],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        // Stays as OR
        assert!(result.is_none());
        assert_eq!(expr.operands.len(), 2);

        // One InList, one arg
        let has_in_list = expr.operands.iter().any(|e| matches!(e, Expr::InList(_)));
        let has_arg = expr.operands.iter().any(|e| matches!(e, Expr::Arg(_)));
        assert!(has_in_list);
        assert!(has_arg);
    }

    #[test]
    fn or_to_in_non_const_rhs_not_converted() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `a = b or a = c` (non-constant RHS, not converted)
        let mut expr = ExprOr {
            operands: vec![
                Expr::eq(Expr::arg(0), Expr::arg(1)),
                Expr::eq(Expr::arg(0), Expr::arg(2)),
            ],
        };
        let result = simplify.simplify_expr_or(&mut expr);

        // Not converted, stays as OR
        assert!(result.is_none());
        assert_eq!(expr.operands.len(), 2);
        assert!(expr.operands.iter().all(|e| matches!(e, Expr::BinaryOp(_))));
    }
}

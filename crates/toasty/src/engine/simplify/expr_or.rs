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
}

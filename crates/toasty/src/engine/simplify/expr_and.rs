use super::Simplify;
use std::mem;
use toasty_core::stmt;

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::simplify::test::test_schema;
    use toasty_core::stmt::{Expr, ExprAnd};

    /// Builds `and(a, and(b, c))`, a nested AND structure for testing
    /// flattening.
    fn nested_and(a: Expr, b: Expr, c: Expr) -> ExprAnd {
        ExprAnd {
            operands: vec![
                a,
                Expr::And(ExprAnd {
                    operands: vec![b, c],
                }),
            ],
        }
    }

    #[test]
    fn flatten_all_symbolic() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `and(A, and(B, C)) → and(A, B, C)`
        let mut expr = nested_and(Expr::arg(0), Expr::arg(1), Expr::arg(2));
        let result = simplify.simplify_expr_and(&mut expr);

        assert!(result.is_none()); // Modified in place
        assert_eq!(expr.operands.len(), 3);
        assert_eq!(expr.operands[0], Expr::arg(0));
        assert_eq!(expr.operands[1], Expr::arg(1));
        assert_eq!(expr.operands[2], Expr::arg(2));
    }

    #[test]
    fn flatten_with_true_in_outer() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `and(true, and(B, C)) → and(B, C)`
        let mut expr = nested_and(true.into(), Expr::arg(1), Expr::arg(2));
        let result = simplify.simplify_expr_and(&mut expr);

        assert!(result.is_none());
        assert_eq!(expr.operands.len(), 2);
        assert_eq!(expr.operands[0], Expr::arg(1));
        assert_eq!(expr.operands[1], Expr::arg(2));
    }

    #[test]
    fn flatten_with_true_in_nested_first() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `and(A, and(true, C)) → and(A, C)`
        let mut expr = nested_and(Expr::arg(0), true.into(), Expr::arg(2));
        let result = simplify.simplify_expr_and(&mut expr);

        assert!(result.is_none());
        assert_eq!(expr.operands.len(), 2);
        assert_eq!(expr.operands[0], Expr::arg(0));
        assert_eq!(expr.operands[1], Expr::arg(2));
    }

    #[test]
    fn flatten_with_true_in_nested_second() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `and(A, and(B, true)) → and(A, B)`
        let mut expr = nested_and(Expr::arg(0), Expr::arg(1), true.into());
        let result = simplify.simplify_expr_and(&mut expr);

        assert!(result.is_none());
        assert_eq!(expr.operands.len(), 2);
        assert_eq!(expr.operands[0], Expr::arg(0));
        assert_eq!(expr.operands[1], Expr::arg(1));
    }

    #[test]
    fn flatten_outer_true_nested_one_true() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `and(true, and(true, C)) → C`
        let mut expr = nested_and(true.into(), true.into(), Expr::arg(2));
        let result = simplify.simplify_expr_and(&mut expr);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), Expr::arg(2));
    }

    #[test]
    fn flatten_outer_symbolic_nested_all_true() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `and(A, and(true, true)) → A`
        let mut expr = nested_and(Expr::arg(0), true.into(), true.into());
        let result = simplify.simplify_expr_and(&mut expr);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), Expr::arg(0));
    }

    #[test]
    fn flatten_all_true() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `and(true, and(true, true)) → true`
        let mut expr = nested_and(true.into(), true.into(), true.into());
        let result = simplify.simplify_expr_and(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_true());
    }

    #[test]
    fn flatten_with_false_in_outer() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `and(false, and(B, C)) → false`
        let mut expr = nested_and(false.into(), Expr::arg(1), Expr::arg(2));
        let result = simplify.simplify_expr_and(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_false());
    }

    #[test]
    fn flatten_with_false_in_nested() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `and(A, and(false, C)) → false`
        let mut expr = nested_and(Expr::arg(0), false.into(), Expr::arg(2));
        let result = simplify.simplify_expr_and(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_false());
    }

    #[test]
    fn flatten_true_and_false_mixed() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `and(true, and(false, true)) → false`
        let mut expr = nested_and(true.into(), false.into(), true.into());
        let result = simplify.simplify_expr_and(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_false());
    }

    #[test]
    fn single_operand_unwrapped() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `and(arg(0)) → arg(0)`
        let mut expr = ExprAnd {
            operands: vec![Expr::arg(0)],
        };
        let result = simplify.simplify_expr_and(&mut expr);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), Expr::arg(0));
    }

    #[test]
    fn empty_after_removing_true() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `and(true, true) → true`
        let mut expr = ExprAnd {
            operands: vec![true.into(), true.into()],
        };
        let result = simplify.simplify_expr_and(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_true());
    }

    #[test]
    fn idempotent_two_identical() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `and(a, a) → a`
        let mut expr = ExprAnd {
            operands: vec![Expr::arg(0), Expr::arg(0)],
        };
        let result = simplify.simplify_expr_and(&mut expr);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), Expr::arg(0));
    }

    #[test]
    fn idempotent_three_identical() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `and(a, a, a) → a`
        let mut expr = ExprAnd {
            operands: vec![Expr::arg(0), Expr::arg(0), Expr::arg(0)],
        };
        let result = simplify.simplify_expr_and(&mut expr);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), Expr::arg(0));
    }

    #[test]
    fn idempotent_with_different() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `and(a, b, a) → and(a, b)`
        let mut expr = ExprAnd {
            operands: vec![Expr::arg(0), Expr::arg(1), Expr::arg(0)],
        };
        let result = simplify.simplify_expr_and(&mut expr);

        assert!(result.is_none());
        assert_eq!(expr.operands.len(), 2);
        assert_eq!(expr.operands[0], Expr::arg(0));
        assert_eq!(expr.operands[1], Expr::arg(1));
    }

    #[test]
    fn absorption_and_or() {
        use toasty_core::stmt::ExprOr;

        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `and(a, or(a, b))` → `a`
        let mut expr = ExprAnd {
            operands: vec![
                Expr::arg(0),
                Expr::Or(ExprOr {
                    operands: vec![Expr::arg(0), Expr::arg(1)],
                }),
            ],
        };
        let result = simplify.simplify_expr_and(&mut expr);

        assert!(result.is_some());
        assert_eq!(result.unwrap(), Expr::arg(0));
    }

    #[test]
    fn absorption_with_multiple_operands() {
        use toasty_core::stmt::ExprOr;

        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `and(a, b, or(a, c))` → `and(a, b)`
        let mut expr = ExprAnd {
            operands: vec![
                Expr::arg(0),
                Expr::arg(1),
                Expr::Or(ExprOr {
                    operands: vec![Expr::arg(0), Expr::arg(2)],
                }),
            ],
        };
        let result = simplify.simplify_expr_and(&mut expr);

        assert!(result.is_none());
        assert_eq!(expr.operands.len(), 2);
        assert_eq!(expr.operands[0], Expr::arg(0));
        assert_eq!(expr.operands[1], Expr::arg(1));
    }

    #[test]
    fn absorption_two_and_three_or() {
        use toasty_core::stmt::ExprOr;

        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `and(a, b, or(a, c, d))` → `and(a, b)`
        let mut expr = ExprAnd {
            operands: vec![
                Expr::arg(0),
                Expr::arg(1),
                Expr::Or(ExprOr {
                    operands: vec![Expr::arg(0), Expr::arg(2), Expr::arg(3)],
                }),
            ],
        };
        let result = simplify.simplify_expr_and(&mut expr);

        assert!(result.is_none());
        assert_eq!(expr.operands.len(), 2);
        assert_eq!(expr.operands[0], Expr::arg(0));
        assert_eq!(expr.operands[1], Expr::arg(1));
    }

    #[test]
    fn complement_basic() {
        use toasty_core::stmt::ExprNot;

        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `a and not(a)` → `false` (where a is a non-nullable comparison)
        let a = Expr::eq(Expr::arg(0), Expr::arg(1));
        let mut expr = ExprAnd {
            operands: vec![a.clone(), Expr::Not(ExprNot { expr: Box::new(a) })],
        };
        let result = simplify.simplify_expr_and(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_false());
    }

    #[test]
    fn complement_with_other_operands() {
        use toasty_core::stmt::ExprNot;

        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `a and b and not(a)` → `false`
        let a = Expr::eq(Expr::arg(0), Expr::arg(1));
        let mut expr = ExprAnd {
            operands: vec![
                a.clone(),
                Expr::arg(2),
                Expr::Not(ExprNot { expr: Box::new(a) }),
            ],
        };
        let result = simplify.simplify_expr_and(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_false());
    }

    #[test]
    fn complement_nullable_not_simplified() {
        use toasty_core::stmt::ExprNot;

        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `a and not(a)` where `a` is an arg (nullable) → no change
        let a = Expr::arg(0);
        let mut expr = ExprAnd {
            operands: vec![a.clone(), Expr::Not(ExprNot { expr: Box::new(a) })],
        };
        let result = simplify.simplify_expr_and(&mut expr);

        assert!(result.is_none());
    }

    #[test]
    fn complement_multiple_repetitions() {
        use toasty_core::stmt::ExprNot;

        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `a and a and not(a) and not(a)` → `false`
        let a = Expr::eq(Expr::arg(0), Expr::arg(1));
        let mut expr = ExprAnd {
            operands: vec![
                a.clone(),
                a.clone(),
                Expr::Not(ExprNot {
                    expr: Box::new(a.clone()),
                }),
                Expr::Not(ExprNot { expr: Box::new(a) }),
            ],
        };
        let result = simplify.simplify_expr_and(&mut expr);

        assert!(result.is_some());
        assert!(result.unwrap().is_false());
    }
}

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

        if expr.operands.is_empty() {
            Some(false.into())
        } else if expr.operands.len() == 1 {
            Some(expr.operands.remove(0))
        } else {
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
}

use super::Simplify;
use std::mem;
use toasty_core::stmt;

impl Simplify<'_> {
    pub(super) fn simplify_expr_and(&mut self, expr: &mut stmt::ExprAnd) -> Option<stmt::Expr> {
        // First, flatten any nested ands
        for i in 0..expr.operands.len() {
            if let stmt::Expr::And(and) = &mut expr.operands[i] {
                let mut nested = mem::take(&mut and.operands);
                expr.operands[i] = true.into();
                expr.operands.append(&mut nested);
            }
        }

        expr.operands.retain(|expr| !expr.is_true());

        if expr.operands.is_empty() {
            Some(true.into())
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
    use toasty_core::stmt::{Expr, ExprAnd};

    /// Builds `and(a, and(b, c))`, a nested AND structure for testing flattening.
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
    fn flatten_with_false_preserved() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `and(false, and(B, C)) → and(false, B, C)`, false is NOT removed
        let mut expr = nested_and(false.into(), Expr::arg(1), Expr::arg(2));
        let result = simplify.simplify_expr_and(&mut expr);

        assert!(result.is_none());
        assert_eq!(expr.operands.len(), 3);
        assert!(expr.operands[0].is_false());
        assert_eq!(expr.operands[1], Expr::arg(1));
        assert_eq!(expr.operands[2], Expr::arg(2));
    }

    #[test]
    fn flatten_with_false_in_nested() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `and(A, and(false, C)) → and(A, false, C)`
        let mut expr = nested_and(Expr::arg(0), false.into(), Expr::arg(2));
        let result = simplify.simplify_expr_and(&mut expr);

        assert!(result.is_none());
        assert_eq!(expr.operands.len(), 3);
        assert_eq!(expr.operands[0], Expr::arg(0));
        assert!(expr.operands[1].is_false());
        assert_eq!(expr.operands[2], Expr::arg(2));
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
}

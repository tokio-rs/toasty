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

        // Range to equality: `a >= c and a <= c` → `a = c`
        self.try_range_to_equality(expr);

        if expr.operands.is_empty() {
            Some(true.into())
        } else if expr.operands.len() == 1 {
            Some(expr.operands.remove(0))
        } else {
            None
        }
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
    fn range_to_equality_ge_le() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `a >= 5 and a <= 5` → `a = 5`
        let mut expr = ExprAnd {
            operands: vec![
                Expr::binary_op(Expr::arg(0), BinaryOp::Ge, 5i64),
                Expr::binary_op(Expr::arg(0), BinaryOp::Le, 5i64),
            ],
        };
        let result = simplify.simplify_expr_and(&mut expr);

        let Some(Expr::BinaryOp(bin_op)) = result else {
            panic!("expected binary op");
        };
        assert!(bin_op.op.is_eq());
    }

    #[test]
    fn range_to_equality_le_ge() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `a <= 5 and a >= 5` → `a = 5` (opposite order)
        let mut expr = ExprAnd {
            operands: vec![
                Expr::binary_op(Expr::arg(0), BinaryOp::Le, 5i64),
                Expr::binary_op(Expr::arg(0), BinaryOp::Ge, 5i64),
            ],
        };
        let result = simplify.simplify_expr_and(&mut expr);

        let Some(Expr::BinaryOp(bin_op)) = result else {
            panic!("expected binary op");
        };
        assert!(bin_op.op.is_eq());
    }

    #[test]
    fn range_to_equality_different_bounds_not_simplified() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `a >= 5 and a <= 10` is not simplified (different bounds)
        let mut expr = ExprAnd {
            operands: vec![
                Expr::binary_op(Expr::arg(0), BinaryOp::Ge, 5i64),
                Expr::binary_op(Expr::arg(0), BinaryOp::Le, 10i64),
            ],
        };
        let result = simplify.simplify_expr_and(&mut expr);

        assert!(result.is_none());
        assert_eq!(expr.operands.len(), 2);
    }

    #[test]
    fn range_to_equality_different_exprs_not_simplified() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `a >= 5 and b <= 5` is not simplified (different expressions)
        let mut expr = ExprAnd {
            operands: vec![
                Expr::binary_op(Expr::arg(0), BinaryOp::Ge, 5i64),
                Expr::binary_op(Expr::arg(1), BinaryOp::Le, 5i64),
            ],
        };
        let result = simplify.simplify_expr_and(&mut expr);

        assert!(result.is_none());
        assert_eq!(expr.operands.len(), 2);
    }

    #[test]
    fn range_to_equality_with_other_operands() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `x and a >= 5 and a <= 5` → `x and a = 5`
        let mut expr = ExprAnd {
            operands: vec![
                Expr::arg(0),
                Expr::binary_op(Expr::arg(1), BinaryOp::Ge, 5i64),
                Expr::binary_op(Expr::arg(1), BinaryOp::Le, 5i64),
            ],
        };
        let result = simplify.simplify_expr_and(&mut expr);

        assert!(result.is_none()); // Still has multiple operands
        assert_eq!(expr.operands.len(), 2);

        // One should be arg(0), the other should be the equality
        let has_equality = expr
            .operands
            .iter()
            .any(|e| matches!(e, Expr::BinaryOp(op) if op.op.is_eq()));
        assert!(has_equality);
    }

    #[test]
    fn range_to_equality_uneven_repetitions() {
        let schema = test_schema();
        let mut simplify = Simplify::new(&schema);

        // `a >= 5 and a >= 5 and a <= 5` → `a = 5`
        let mut expr = ExprAnd {
            operands: vec![
                Expr::binary_op(Expr::arg(0), BinaryOp::Ge, 5i64),
                Expr::binary_op(Expr::arg(0), BinaryOp::Ge, 5i64),
                Expr::binary_op(Expr::arg(0), BinaryOp::Le, 5i64),
            ],
        };
        let result = simplify.simplify_expr_and(&mut expr);

        // All bounds collapse to a single equality
        let Some(Expr::BinaryOp(bin_op)) = result else {
            panic!("expected binary op");
        };
        assert!(bin_op.op.is_eq());
    }
}

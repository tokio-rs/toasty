use super::Simplify;
use toasty_core::stmt::{self, Expr};

impl Simplify<'_> {
    pub(super) fn simplify_expr_unary_op(&self, expr: &mut stmt::ExprUnaryOp) -> Option<Expr> {
        match expr.op {
            stmt::UnaryOp::Neg => self.simplify_neg(expr),
        }
    }

    fn simplify_neg(&self, expr: &mut stmt::ExprUnaryOp) -> Option<Expr> {
        // Double negation: `--x` → `x`
        if let Expr::UnaryOp(inner) = expr.expr.as_mut() {
            if matches!(inner.op, stmt::UnaryOp::Neg) {
                return Some(inner.expr.take());
            }
        }

        // Null propagation: `-null` → `null`
        if expr.expr.is_value_null() {
            return Some(Expr::null());
        }

        // Constant folding: `-5` → `-5` as value
        if let Expr::Value(val) = expr.expr.as_ref() {
            return fold_neg(val);
        }

        None
    }
}

/// Folds negation of constant values.
///
/// Returns `None` if the negation would overflow.
fn fold_neg(val: &stmt::Value) -> Option<Expr> {
    let result = match val {
        stmt::Value::I8(v) => stmt::Value::I8(v.checked_neg()?),
        stmt::Value::I16(v) => stmt::Value::I16(v.checked_neg()?),
        stmt::Value::I32(v) => stmt::Value::I32(v.checked_neg()?),
        stmt::Value::I64(v) => stmt::Value::I64(v.checked_neg()?),
        _ => return None,
    };
    Some(Expr::Value(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::simplify::test::test_schema;
    use toasty_core::stmt::{ExprUnaryOp, UnaryOp, Value};

    fn neg_expr(expr: Expr) -> ExprUnaryOp {
        ExprUnaryOp {
            op: UnaryOp::Neg,
            expr: Box::new(expr),
        }
    }

    #[test]
    fn double_negation_eliminated() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `--x` → `x`
        let inner = Expr::neg(Expr::arg(0));
        let mut expr = neg_expr(inner);

        let result = simplify.simplify_expr_unary_op(&mut expr);

        assert!(matches!(result, Some(Expr::Arg(_))));
    }

    #[test]
    fn triple_negation_reduces_to_single() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `---x` → `-x`
        let inner = Expr::neg(Expr::neg(Expr::arg(0)));
        let mut expr = neg_expr(inner);

        let result = simplify.simplify_expr_unary_op(&mut expr);

        assert!(matches!(result, Some(Expr::UnaryOp(_))));
    }

    #[test]
    fn neg_null_becomes_null() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `-null` → `null`
        let mut expr = neg_expr(Expr::null());

        let result = simplify.simplify_expr_unary_op(&mut expr);

        assert!(matches!(result, Some(Expr::Value(Value::Null))));
    }

    #[test]
    fn neg_constant_folding() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `-5` → `-5` as value
        let mut expr = neg_expr(Expr::Value(Value::I64(5)));

        let result = simplify.simplify_expr_unary_op(&mut expr);

        assert!(matches!(result, Some(Expr::Value(Value::I64(-5)))));
    }

    #[test]
    fn neg_zero_stays_zero() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `-0` → `0`
        let mut expr = neg_expr(Expr::Value(Value::I64(0)));

        let result = simplify.simplify_expr_unary_op(&mut expr);

        assert!(matches!(result, Some(Expr::Value(Value::I64(0)))));
    }

    #[test]
    fn neg_negative_becomes_positive() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `-(-3)` → `3` (constant in literal)
        let mut expr = neg_expr(Expr::Value(Value::I64(-3)));

        let result = simplify.simplify_expr_unary_op(&mut expr);

        assert!(matches!(result, Some(Expr::Value(Value::I64(3)))));
    }

    #[test]
    fn neg_i32_constant_folding() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `-5i32` → `-5i32` as value
        let mut expr = neg_expr(Expr::Value(Value::I32(5)));

        let result = simplify.simplify_expr_unary_op(&mut expr);

        assert!(matches!(result, Some(Expr::Value(Value::I32(-5)))));
    }

    #[test]
    fn neg_non_constant_not_simplified() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `-x` where x is not a constant
        let mut expr = neg_expr(Expr::arg(0));

        let result = simplify.simplify_expr_unary_op(&mut expr);

        assert!(result.is_none());
    }

    #[test]
    fn neg_i8_min_not_simplified() {
        let schema = test_schema();
        let simplify = Simplify::new(&schema);

        // `-i8::MIN` would overflow, so not simplified
        let mut expr = neg_expr(Expr::Value(Value::I8(i8::MIN)));

        let result = simplify.simplify_expr_unary_op(&mut expr);

        assert!(result.is_none());
    }
}

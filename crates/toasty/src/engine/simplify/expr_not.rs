use super::Simplify;
use toasty_core::stmt::{self, Expr};

impl Simplify<'_> {
    pub(super) fn simplify_expr_not(&mut self, expr_not: &mut stmt::ExprNot) -> Option<Expr> {
        // Double negation elimination, `not(not(x))` → `x`
        if let Expr::Not(inner) = expr_not.expr.as_mut() {
            return Some(inner.expr.take());
        }

        // Constant folding,
        //
        //   - `not(true)` → `false`
        //   - `not(false)` → `true`
        //   - `not(null)` → `null`
        match expr_not.expr.as_ref() {
            Expr::Value(stmt::Value::Bool(b)) => {
                return Some(Expr::Value(stmt::Value::Bool(!b)));
            }
            Expr::Value(stmt::Value::Null) => {
                return Some(Expr::null());
            }
            _ => {}
        }

        // Negation of comparisons, `not(x = y)` → `x != y`, etc.
        if let Expr::BinaryOp(binary_op) = expr_not.expr.as_mut() {
            if let Some(negated_op) = binary_op.op.negate() {
                binary_op.op = negated_op;
                return Some(expr_not.expr.take());
            }
        }

        // De Morgan's law, `not(a and b)` → `not(a) or not(b)`
        if let Expr::And(expr_and) = expr_not.expr.as_mut() {
            let negated = expr_and
                .operands
                .drain(..)
                .map(Expr::not)
                .collect::<Vec<_>>();
            return Some(Expr::or_from_vec(negated));
        }

        // De Morgan's law, `not(a or b)` → `not(a) and not(b)`
        if let Expr::Or(expr_or) = expr_not.expr.as_mut() {
            let negated = expr_or
                .operands
                .drain(..)
                .map(Expr::not)
                .collect::<Vec<_>>();
            return Some(Expr::and_from_vec(negated));
        }

        // `not(x in ())` → `true` (x NOT IN empty list is always true)
        if let Expr::InList(expr_in_list) = expr_not.expr.as_ref() {
            if expr_in_list.list.is_list_empty() {
                return Some(true.into());
            }
        }

        None
    }
}

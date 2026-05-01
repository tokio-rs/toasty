use std::cmp::PartialOrd;
use toasty_core::stmt::{self, BinaryOp, Expr};

/// Cheap canonicalization for binary ops: constant folding, null
/// propagation, boolean-constant simplification, literal-on-right swap.
///
/// Heavyweight rules (self-comparison with field nullability, tuple
/// decomposition, match elimination, derived-column null check, relation
/// path lifting) live in `simplify/expr_binary_op.rs` and run after this
/// fold pass on canonical input.
pub(super) fn fold_expr_binary_op(op: BinaryOp, lhs: &mut Expr, rhs: &mut Expr) -> Option<Expr> {
    match (&mut *lhs, &mut *rhs) {
        // Constant folding and null propagation:
        //
        //  - `5 = 5` → `true`
        //  - `1 < 5` → `true`
        //  - `"a" >= "b"` → `false`
        //  - `null <op> x` → `null`
        //  - `x <op> null` → `null`
        (Expr::Value(lhs_val), Expr::Value(rhs_val)) => {
            if lhs_val.is_null() || rhs_val.is_null() {
                return Some(Expr::null());
            }

            match op {
                BinaryOp::Eq => Some((*lhs_val == *rhs_val).into()),
                BinaryOp::Ne => Some((*lhs_val != *rhs_val).into()),
                BinaryOp::Lt => {
                    PartialOrd::partial_cmp(&*lhs_val, &*rhs_val).map(|o| o.is_lt().into())
                }
                BinaryOp::Le => {
                    PartialOrd::partial_cmp(&*lhs_val, &*rhs_val).map(|o| o.is_le().into())
                }
                BinaryOp::Gt => {
                    PartialOrd::partial_cmp(&*lhs_val, &*rhs_val).map(|o| o.is_gt().into())
                }
                BinaryOp::Ge => {
                    PartialOrd::partial_cmp(&*lhs_val, &*rhs_val).map(|o| o.is_ge().into())
                }
            }
        }
        // Boolean constant comparisons:
        //
        //  - `x = true` → `x`
        //  - `x = false` → `not(x)`
        //  - `x != true` → `not(x)`
        //  - `x != false` → `x`
        (expr, Expr::Value(stmt::Value::Bool(b))) | (Expr::Value(stmt::Value::Bool(b)), expr)
            if op.is_eq() || op.is_ne() =>
        {
            let is_eq_true = (op.is_eq() && *b) || (op.is_ne() && !*b);
            if is_eq_true {
                Some(expr.take())
            } else {
                Some(Expr::not(expr.take()))
            }
        }
        // Null propagation: `expr <op> null` → `null` (and symmetric).
        // SQL three-valued logic: any comparison with NULL yields NULL.
        (_, Expr::Value(stmt::Value::Null)) | (Expr::Value(stmt::Value::Null), _) => {
            Some(Expr::null())
        }
        // Canonicalization, `literal <op> col` → `col <op_commuted> literal`.
        // Heavyweight rules can then assume the literal (when present) is on
        // the right.
        (Expr::Value(_), rhs) if !rhs.is_value() => {
            std::mem::swap(lhs, rhs);
            Some(Expr::binary_op(lhs.take(), op.commute(), rhs.take()))
        }
        _ => None,
    }
}

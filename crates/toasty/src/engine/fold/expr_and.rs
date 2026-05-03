use std::mem;
use toasty_core::stmt::{self, Expr};

/// Cheap canonicalization for AND: flatten nested ANDs, short-circuit on
/// `false`, drop `true`, propagate all-null, collapse single/empty.
///
/// Heavyweight rules (idempotent law, absorption, complement, range-to-eq,
/// contradiction, OR-branch pruning) live in `simplify/expr_and.rs` and run
/// after this fold pass on canonical input.
pub(super) fn fold_expr_and(expr: &mut stmt::ExprAnd) -> Option<Expr> {
    // Flatten nested ANDs in place. Marker `true` literals are dropped by
    // the next step.
    for i in 0..expr.operands.len() {
        if let Expr::And(and) = &mut expr.operands[i] {
            let mut nested = mem::take(&mut and.operands);
            expr.operands[i] = true.into();
            expr.operands.append(&mut nested);
        }
    }

    // `and(..., false, ...)` → `false`
    if expr.operands.iter().any(|e| e.is_false()) {
        return Some(false.into());
    }

    // `and(..., true, ...)` → `and(..., ...)`
    expr.operands.retain(|operand| !operand.is_true());

    // Null propagation: all remaining operands are null literals.
    if !expr.operands.is_empty() && expr.operands.iter().all(|e| e.is_value_null()) {
        return Some(Expr::null());
    }

    // Empty/single-operand collapse.
    if expr.operands.is_empty() {
        Some(true.into())
    } else if expr.operands.len() == 1 {
        Some(expr.operands.remove(0))
    } else {
        None
    }
}

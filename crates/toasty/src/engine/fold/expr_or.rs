use std::mem;
use toasty_core::stmt::{self, Expr};

/// Cheap canonicalization for OR: flatten nested ORs, short-circuit on
/// `true`, drop `false`, propagate all-null, collapse single/empty.
///
/// Heavyweight rules (idempotent law, absorption, factoring, complement,
/// variant tautology, OR-to-IN) live in `simplify/expr_or.rs` and run
/// after this fold pass on canonical input.
pub(super) fn fold_expr_or(expr: &mut stmt::ExprOr) -> Option<Expr> {
    // Flatten nested ORs in place. Marker `false` literals are dropped by
    // the next step.
    for i in 0..expr.operands.len() {
        if let Expr::Or(or) = &mut expr.operands[i] {
            let mut nested = mem::take(&mut or.operands);
            expr.operands[i] = false.into();
            expr.operands.append(&mut nested);
        }
    }

    // `or(..., true, ...)` → `true`
    if expr.operands.iter().any(|e| e.is_true()) {
        return Some(true.into());
    }

    // `or(..., false, ...)` → `or(..., ...)`
    expr.operands.retain(|operand| !operand.is_false());

    // Null propagation: all remaining operands are null literals.
    if !expr.operands.is_empty() && expr.operands.iter().all(|e| e.is_value_null()) {
        return Some(Expr::null());
    }

    // Empty/single-operand collapse.
    if expr.operands.is_empty() {
        Some(false.into())
    } else if expr.operands.len() == 1 {
        Some(expr.operands.remove(0))
    } else {
        None
    }
}

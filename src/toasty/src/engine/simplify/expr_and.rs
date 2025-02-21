use super::*;

use std::mem;

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

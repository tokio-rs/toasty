/*
use super::*;

pub(crate) fn apply<'stmt>(expr: &mut stmt::Expr<'stmt>) {
    match expr {
        stmt::Expr::And(expr_and) => {
            expr_and.operands.retain(|operand| !operand.is_true());

            if expr_and.operands.is_empty() {
                *expr = true.into();
            } else if expr_and.operands.len() == 1 {
                let e = expr_and.operands.drain(..).next().unwrap();
                *expr = e;
            }
        }
        stmt::Expr::Or(_) => {
            todo!()
        }
        _ => {}
    }
}
*/

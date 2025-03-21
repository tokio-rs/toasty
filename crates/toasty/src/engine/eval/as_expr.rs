use super::*;

pub(crate) trait AsExpr {
    fn as_expr(&self) -> &stmt::Expr;
}

impl AsExpr for stmt::Expr {
    fn as_expr(&self) -> &stmt::Expr {
        self
    }
}

impl AsExpr for &stmt::Expr {
    fn as_expr(&self) -> &stmt::Expr {
        self
    }
}

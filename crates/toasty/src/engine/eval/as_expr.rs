use toasty_core::stmt;

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

impl AsExpr for stmt::Filter {
    fn as_expr(&self) -> &stmt::Expr {
        self.as_expr()
    }
}

impl AsExpr for &stmt::Filter {
    fn as_expr(&self) -> &stmt::Expr {
        stmt::Filter::as_expr(self)
    }
}

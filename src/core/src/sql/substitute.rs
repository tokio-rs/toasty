use expr_placeholder::ExprPlaceholder;

use super::*;

pub trait Input<'stmt> {
    fn resolve_placeholder(&mut self, expr_placeholder: &ExprPlaceholder) -> Expr<'stmt>;
}

impl<'stmt> Input<'stmt> for &mut [Option<Expr<'stmt>>] {
    fn resolve_placeholder(&mut self, expr_placeholder: &ExprPlaceholder) -> Expr<'stmt> {
        self[expr_placeholder.position].take().expect("no argument")
    }
}

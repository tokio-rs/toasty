use super::*;

#[derive(Debug, Clone)]
pub struct ExprList<'stmt> {
    pub items: Vec<Expr<'stmt>>,
}

impl<'stmt> Expr<'stmt> {
    pub fn list_from_vec(items: Vec<Expr<'stmt>>) -> Expr<'stmt> {
        ExprList { items }.into()
    }
}

impl<'stmt> ExprList<'stmt> {
    pub(crate) fn from_stmt(
        stmt: Vec<stmt::Expr<'stmt>>,
        convert: &mut impl Convert<'stmt>,
    ) -> ExprList<'stmt> {
        ExprList {
            items: stmt
                .into_iter()
                .map(|expr| Expr::from_stmt_by_ref(expr, convert))
                .collect(),
        }
    }
}

impl<'stmt> From<ExprList<'stmt>> for Expr<'stmt> {
    fn from(value: ExprList<'stmt>) -> Self {
        Expr::List(value)
    }
}

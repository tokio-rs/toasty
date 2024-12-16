use super::*;

#[derive(Debug, Clone)]
pub struct ExprList {
    pub items: Vec<Expr>,
}

impl Expr {
    pub fn list_from_vec(items: Vec<Expr>) -> Expr {
        ExprList { items }.into()
    }
}

impl ExprList {
    pub(crate) fn from_stmt(stmt: stmt::ExprList, convert: &mut impl Convert) -> ExprList {
        ExprList {
            items: stmt
                .items
                .into_iter()
                .map(|expr| Expr::from_stmt_by_ref(expr, convert))
                .collect(),
        }
    }
}

impl From<ExprList> for Expr {
    fn from(value: ExprList) -> Self {
        Expr::List(value)
    }
}

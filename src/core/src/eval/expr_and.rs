use super::*;

#[derive(Debug, Clone)]
pub struct ExprAnd<'stmt> {
    pub operands: Vec<Expr<'stmt>>,
}

impl<'stmt> ExprAnd<'stmt> {
    pub(crate) fn from_stmt(stmt: stmt::ExprAnd<'stmt>) -> ExprAnd<'stmt> {
        ExprAnd {
            operands: stmt.operands.into_iter().map(Expr::from_stmt).collect(),
        }
    }
}

impl<'stmt> From<ExprAnd<'stmt>> for Expr<'stmt> {
    fn from(value: ExprAnd<'stmt>) -> Self {
        Expr::And(value)
    }
}

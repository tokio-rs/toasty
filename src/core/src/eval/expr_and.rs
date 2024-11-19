use super::*;

#[derive(Debug, Clone)]
pub struct ExprAnd {
    pub operands: Vec<Expr>,
}

impl ExprAnd {
    pub(crate) fn from_stmt(stmt: stmt::ExprAnd, convert: &mut impl Convert) -> ExprAnd {
        ExprAnd {
            operands: stmt
                .operands
                .into_iter()
                .map(|expr| Expr::from_stmt_by_ref(expr, convert))
                .collect(),
        }
    }
}

impl From<ExprAnd> for Expr {
    fn from(value: ExprAnd) -> Self {
        Expr::And(value)
    }
}

use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprInList<'stmt> {
    pub expr: Box<Expr<'stmt>>,
    pub list: Box<Expr<'stmt>>,
}

impl<'stmt> Expr<'stmt> {
    pub fn in_list(lhs: impl Into<Expr<'stmt>>, rhs: impl Into<Expr<'stmt>>) -> Expr<'stmt> {
        ExprInList {
            expr: Box::new(lhs.into()),
            list: Box::new(rhs.into()),
        }
        .into()
    }
}

impl<'stmt> ExprInList<'stmt> {
    pub(crate) fn simplify(&mut self) -> Option<Expr<'stmt>> {
        use std::mem;

        let rhs = match &mut *self.list {
            Expr::Value(value) => {
                let record = value.expect_record();

                if record.len() != 1 {
                    return None;
                }

                Expr::Value(record[0].clone().into_owned())
            }
            Expr::Record(expr_record) => {
                if expr_record.len() != 1 {
                    return None;
                }

                mem::take(&mut expr_record[0])
            }
            _ => return None,
        };

        let lhs = mem::take(&mut *self.expr);

        Some(Expr::eq(lhs, rhs))
    }
}

impl<'stmt> From<ExprInList<'stmt>> for Expr<'stmt> {
    fn from(value: ExprInList<'stmt>) -> Self {
        Expr::InList(value)
    }
}

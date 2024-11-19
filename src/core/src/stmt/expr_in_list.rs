use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprInList {
    pub expr: Box<Expr>,
    pub list: Box<Expr>,
}

impl Expr {
    pub fn in_list(lhs: impl Into<Expr>, rhs: impl Into<Expr>) -> Expr {
        ExprInList {
            expr: Box::new(lhs.into()),
            list: Box::new(rhs.into()),
        }
        .into()
    }
}

impl ExprInList {
    pub(crate) fn simplify(&mut self) -> Option<Expr> {
        use std::mem;

        let rhs = match &mut *self.list {
            Expr::Value(value) => {
                let record = value.expect_record();

                if record.len() != 1 {
                    return None;
                }

                Expr::Value(record[0].clone())
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

impl From<ExprInList> for Expr {
    fn from(value: ExprInList) -> Self {
        Expr::InList(value)
    }
}

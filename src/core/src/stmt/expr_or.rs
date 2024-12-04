use super::*;

use std::ops;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprOr {
    pub operands: Vec<Expr>,
}

impl Expr {
    pub fn or(lhs: impl Into<Expr>, rhs: impl Into<Expr>) -> Expr {
        let mut lhs = lhs.into();
        let rhs = rhs.into();

        match (&mut lhs, rhs) {
            (Expr::Or(lhs_or), Expr::Or(rhs_or)) => {
                lhs_or.operands.extend(rhs_or.operands);
                lhs
            }
            (Expr::Or(lhs_or), rhs) => {
                lhs_or.operands.push(rhs);
                lhs
            }
            (_, Expr::Or(mut lhs_or)) => {
                lhs_or.operands.push(lhs);
                lhs_or.into()
            }
            (_, rhs) => ExprOr {
                operands: vec![lhs, rhs],
            }
            .into(),
        }
    }
}

impl ops::Deref for ExprOr {
    type Target = [Expr];

    fn deref(&self) -> &Self::Target {
        self.operands.deref()
    }
}

impl<'a> IntoIterator for &'a ExprOr {
    type IntoIter = std::slice::Iter<'a, Expr>;
    type Item = &'a Expr;

    fn into_iter(self) -> Self::IntoIter {
        self.operands.iter()
    }
}

impl<'a> IntoIterator for &'a mut ExprOr {
    type IntoIter = std::slice::IterMut<'a, Expr>;
    type Item = &'a mut Expr;

    fn into_iter(self) -> Self::IntoIter {
        self.operands.iter_mut()
    }
}

impl From<ExprOr> for Expr {
    fn from(value: ExprOr) -> Self {
        Expr::Or(value)
    }
}

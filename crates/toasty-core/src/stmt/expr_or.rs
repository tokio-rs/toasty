use super::*;

use std::ops;

#[derive(Debug, Clone)]
pub struct ExprOr {
    pub operands: Vec<Expr>,
}

impl Expr {
    pub fn or(lhs: impl Into<Self>, rhs: impl Into<Self>) -> Self {
        let mut lhs = lhs.into();
        let rhs = rhs.into();

        match (&mut lhs, rhs) {
            (Self::Or(lhs_or), Self::Or(rhs_or)) => {
                lhs_or.operands.extend(rhs_or.operands);
                lhs
            }
            (Self::Or(lhs_or), rhs) => {
                lhs_or.operands.push(rhs);
                lhs
            }
            (_, Self::Or(mut lhs_or)) => {
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
        Self::Or(value)
    }
}

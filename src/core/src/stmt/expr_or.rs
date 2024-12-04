use super::*;

use std::ops;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprOr {
    pub operands: Vec<Expr>,
}

impl ExprOr {
    pub fn new(operands: Vec<Expr>) -> ExprOr {
        ExprOr { operands }
    }

    pub fn new_binary<A, B>(lhs: A, rhs: B) -> ExprOr
    where
        A: Into<Expr>,
        B: Into<Expr>,
    {
        ExprOr {
            operands: vec![lhs.into(), rhs.into()],
        }
    }

    pub fn extend(&mut self, rhs: ExprOr) {
        self.operands.extend(rhs.operands);
    }

    pub fn push(&mut self, expr: Expr) {
        self.operands.push(expr);
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

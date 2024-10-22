use super::*;

use std::ops;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprOr<'stmt> {
    pub operands: Vec<Expr<'stmt>>,
}

impl<'stmt> ExprOr<'stmt> {
    pub fn new(operands: Vec<Expr<'stmt>>) -> ExprOr<'stmt> {
        ExprOr { operands }
    }

    pub fn new_binary<A, B>(lhs: A, rhs: B) -> ExprOr<'stmt>
    where
        A: Into<Expr<'stmt>>,
        B: Into<Expr<'stmt>>,
    {
        ExprOr {
            operands: vec![lhs.into(), rhs.into()],
        }
    }

    pub fn extend(&mut self, rhs: ExprOr<'stmt>) {
        self.operands.extend(rhs.operands);
    }

    pub fn push(&mut self, expr: Expr<'stmt>) {
        self.operands.push(expr);
    }
}

impl<'stmt> ops::Deref for ExprOr<'stmt> {
    type Target = [Expr<'stmt>];

    fn deref(&self) -> &Self::Target {
        self.operands.deref()
    }
}

impl<'a, 'stmt> IntoIterator for &'a ExprOr<'stmt> {
    type IntoIter = std::slice::Iter<'a, Expr<'stmt>>;
    type Item = &'a Expr<'stmt>;

    fn into_iter(self) -> Self::IntoIter {
        self.operands.iter()
    }
}

impl<'a, 'stmt> IntoIterator for &'a mut ExprOr<'stmt> {
    type IntoIter = std::slice::IterMut<'a, Expr<'stmt>>;
    type Item = &'a mut Expr<'stmt>;

    fn into_iter(self) -> Self::IntoIter {
        self.operands.iter_mut()
    }
}

impl<'stmt> From<ExprOr<'stmt>> for Expr<'stmt> {
    fn from(value: ExprOr<'stmt>) -> Self {
        Expr::Or(value)
    }
}

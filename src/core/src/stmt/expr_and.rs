use super::*;

use std::ops;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprAnd<'stmt> {
    pub operands: Vec<Expr<'stmt>>,
}

impl<'stmt> ExprAnd<'stmt> {
    pub fn new(operands: Vec<Expr<'stmt>>) -> ExprAnd<'stmt> {
        ExprAnd { operands }
    }
}

impl<'stmt> ops::Deref for ExprAnd<'stmt> {
    type Target = [Expr<'stmt>];

    fn deref(&self) -> &Self::Target {
        self.operands.deref()
    }
}

impl<'a, 'stmt> IntoIterator for &'a ExprAnd<'stmt> {
    type IntoIter = std::slice::Iter<'a, Expr<'stmt>>;
    type Item = &'a Expr<'stmt>;

    fn into_iter(self) -> Self::IntoIter {
        self.operands.iter()
    }
}

impl<'a, 'stmt> IntoIterator for &'a mut ExprAnd<'stmt> {
    type IntoIter = std::slice::IterMut<'a, Expr<'stmt>>;
    type Item = &'a mut Expr<'stmt>;

    fn into_iter(self) -> Self::IntoIter {
        self.operands.iter_mut()
    }
}

impl<'stmt> From<ExprAnd<'stmt>> for Expr<'stmt> {
    fn from(value: ExprAnd<'stmt>) -> Self {
        Expr::And(value)
    }
}

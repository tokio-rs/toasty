use super::*;

use std::ops;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprConcat<'stmt> {
    pub exprs: Vec<Expr<'stmt>>,
}

impl<'stmt> ExprConcat<'stmt> {
    pub fn new(exprs: Vec<Expr<'stmt>>) -> ExprConcat<'stmt> {
        ExprConcat { exprs }
    }

    pub fn extend(&mut self, rhs: ExprConcat<'stmt>) {
        self.exprs.extend(rhs.exprs);
    }

    pub fn push(&mut self, expr: Expr<'stmt>) {
        self.exprs.push(expr);
    }
}

impl<'stmt> ops::Deref for ExprConcat<'stmt> {
    type Target = [Expr<'stmt>];

    fn deref(&self) -> &Self::Target {
        self.exprs.deref()
    }
}

impl<'stmt> IntoIterator for ExprConcat<'stmt> {
    type IntoIter = std::vec::IntoIter<Expr<'stmt>>;
    type Item = Expr<'stmt>;

    fn into_iter(self) -> Self::IntoIter {
        self.exprs.into_iter()
    }
}

impl<'a, 'stmt> IntoIterator for &'a ExprConcat<'stmt> {
    type IntoIter = std::slice::Iter<'a, Expr<'stmt>>;
    type Item = &'a Expr<'stmt>;

    fn into_iter(self) -> Self::IntoIter {
        self.exprs.iter()
    }
}

impl<'a, 'stmt> IntoIterator for &'a mut ExprConcat<'stmt> {
    type IntoIter = std::slice::IterMut<'a, Expr<'stmt>>;
    type Item = &'a mut Expr<'stmt>;

    fn into_iter(self) -> Self::IntoIter {
        self.exprs.iter_mut()
    }
}

impl<'stmt> From<ExprConcat<'stmt>> for Expr<'stmt> {
    fn from(value: ExprConcat<'stmt>) -> Self {
        Expr::Concat(value)
    }
}

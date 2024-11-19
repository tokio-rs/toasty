use super::*;

use std::ops;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprAnd {
    pub operands: Vec<Expr>,
}

impl ExprAnd {
    pub fn new(operands: Vec<Expr>) -> ExprAnd {
        ExprAnd { operands }
    }
}

impl ops::Deref for ExprAnd {
    type Target = [Expr];

    fn deref(&self) -> &Self::Target {
        self.operands.deref()
    }
}

impl<'a> IntoIterator for &'a ExprAnd {
    type IntoIter = std::slice::Iter<'a, Expr>;
    type Item = &'a Expr;

    fn into_iter(self) -> Self::IntoIter {
        self.operands.iter()
    }
}

impl<'a> IntoIterator for &'a mut ExprAnd {
    type IntoIter = std::slice::IterMut<'a, Expr>;
    type Item = &'a mut Expr;

    fn into_iter(self) -> Self::IntoIter {
        self.operands.iter_mut()
    }
}

impl From<ExprAnd> for Expr {
    fn from(value: ExprAnd) -> Self {
        Expr::And(value)
    }
}

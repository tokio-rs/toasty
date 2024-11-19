use super::*;

use std::ops;

#[derive(Debug, Clone, PartialEq)]
pub struct ExprConcat {
    pub exprs: Vec<Expr>,
}

impl ExprConcat {
    pub fn new(exprs: Vec<Expr>) -> ExprConcat {
        ExprConcat { exprs }
    }

    pub fn extend(&mut self, rhs: ExprConcat) {
        self.exprs.extend(rhs.exprs);
    }

    pub fn push(&mut self, expr: Expr) {
        self.exprs.push(expr);
    }
}

impl ops::Deref for ExprConcat {
    type Target = [Expr];

    fn deref(&self) -> &Self::Target {
        self.exprs.deref()
    }
}

impl IntoIterator for ExprConcat {
    type IntoIter = std::vec::IntoIter<Expr>;
    type Item = Expr;

    fn into_iter(self) -> Self::IntoIter {
        self.exprs.into_iter()
    }
}

impl<'a, 'stmt> IntoIterator for &'a ExprConcat {
    type IntoIter = std::slice::Iter<'a, Expr>;
    type Item = &'a Expr;

    fn into_iter(self) -> Self::IntoIter {
        self.exprs.iter()
    }
}

impl<'a, 'stmt> IntoIterator for &'a mut ExprConcat {
    type IntoIter = std::slice::IterMut<'a, Expr>;
    type Item = &'a mut Expr;

    fn into_iter(self) -> Self::IntoIter {
        self.exprs.iter_mut()
    }
}

impl From<ExprConcat> for Expr {
    fn from(value: ExprConcat) -> Self {
        Expr::Concat(value)
    }
}

use super::*;

use std::ops;

#[derive(Debug, Clone)]
pub struct ExprAnd {
    pub operands: Vec<Expr>,
}

impl Expr {
    pub fn and(lhs: impl Into<Self>, rhs: impl Into<Self>) -> Self {
        let mut lhs = lhs.into();
        let rhs = rhs.into();

        match (&mut lhs, rhs) {
            (expr, rhs) if expr.is_true() => rhs,
            (_, expr) if expr.is_true() => lhs,
            (Self::And(lhs_and), Self::And(rhs_and)) => {
                lhs_and.operands.extend(rhs_and.operands);
                lhs
            }
            (Self::And(lhs_and), rhs) => {
                lhs_and.operands.push(rhs);
                lhs
            }
            (_, Self::And(mut rhs_and)) => {
                rhs_and.operands.push(lhs);
                rhs_and.into()
            }
            (_, rhs) => ExprAnd {
                operands: vec![lhs, rhs],
            }
            .into(),
        }
    }

    pub fn and_from_vec(operands: Vec<Self>) -> Self {
        if operands.is_empty() {
            return true.into();
        }

        if operands.len() == 1 {
            return operands.into_iter().next().unwrap();
        }

        ExprAnd { operands }.into()
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
        Self::And(value)
    }
}

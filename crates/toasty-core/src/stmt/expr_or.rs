use super::Expr;
use std::ops;

/// A logical "or" of multiple expressions.
///
/// Returns `true` if at least one operand evaluates to `true`. An `ExprOr`
/// always has at least two operands; use [`Expr::or_from_vec`] which returns
/// `Expr::Value(false)` for empty input and unwraps single-element input.
///
/// # Examples
///
/// ```text
/// or(a, b, c)  // returns `true` if any of `a`, `b`, or `c` is `true`
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ExprOr {
    /// The expressions to "or" together.
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

    pub fn or_from_vec(operands: Vec<Self>) -> Self {
        if operands.is_empty() {
            return false.into();
        }

        if operands.len() == 1 {
            return operands.into_iter().next().unwrap();
        }

        ExprOr { operands }.into()
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

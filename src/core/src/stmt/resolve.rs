use super::*;

use std::ops;

pub(crate) trait ExprSelf<'stmt>:
    ops::Index<PathStep, Output = Expr<'stmt>> + ops::IndexMut<PathStep>
{
}

impl<'stmt, T: ?Sized> ExprSelf<'stmt> for T where
    T: ops::Index<PathStep, Output = Expr<'stmt>> + ops::IndexMut<PathStep>
{
}

impl<'stmt> Expr<'stmt> {
    pub fn resolve<S>(&self, steps: impl IntoIterator<Item = S>) -> &Expr<'stmt>
    where
        S: Into<PathStep>,
    {
        resolve(self, steps).unwrap_or(self)
    }

    /// TODO: can we resolve with above?
    pub fn resolve_mut<S>(&mut self, steps: impl IntoIterator<Item = S>) -> &mut Expr<'stmt>
    where
        S: Into<PathStep>,
    {
        // Unfotunately, we have to duplicate code to make the borrow checker happy.
        let mut ret = self;

        for step in steps {
            ret = &mut ret[step.into()];
        }

        ret
    }
}

pub(crate) fn resolve<'stmt, T, I, S>(expr_self: &T, steps: I) -> Option<&Expr<'stmt>>
where
    T: ExprSelf<'stmt> + ?Sized,
    I: IntoIterator<Item = S>,
    S: Into<PathStep>,
{
    let mut steps = steps.into_iter();

    let Some(first) = steps.next() else {
        return None;
    };

    let mut ret = &expr_self[first.into()];

    for step in steps {
        ret = &ret[step.into()];
    }

    Some(ret)
}

/*
pub(crate) fn resolve_mut<'stmt, T, I, S>(expr_self: &mut T, steps: I) -> Option<&mut Expr<'stmt>>
where
    T: ExprSelf<'stmt> + ?Sized,
    I: IntoIterator<Item = S>,
    S: Into<PathStep>,
{
    let mut steps = steps.into_iter();

    let Some(first) = steps.next() else {
        return None;
    };

    let mut ret = &mut expr_self[first.into()];

    for step in steps {
        ret = &mut ret[step.into()];
    }

    Some(ret)
}
*/
